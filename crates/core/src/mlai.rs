//! ML AI Code control plane: run tokens (one per model), catalog, finish.
use crate::auth::Credential;
use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

pub const DEFAULT_GATEWAY: &str = "https://code.ml.ai/api/model/v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedModel {
    pub id: Option<String>,
    pub context_window: Option<i64>,
    pub response_reserve: Option<i64>,
    pub vision: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResponse {
    pub run_id: Option<String>,
    pub token: Option<String>,
    pub expires: Option<i64>,
    pub gateway: Option<String>,
    pub model: Option<StartedModel>,
    pub budget: Option<i64>,
    pub account_tokens_left: Option<i64>,
    pub intake: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CatalogModel {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub available: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Catalog {
    pub models: Vec<CatalogModel>,
    pub default_model: Option<String>,
}

pub struct MlaiClient {
    pub server: String,
    token: String,
    http: reqwest::Client,
}

impl MlaiClient {
    pub fn new(server: &str, token: &str) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("http client");
        MlaiClient {
            server: server.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        }
    }

    fn request(&self, path: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(reqwest::Method::POST, format!("{}{}", self.server, path))
            .header("Content-Type", "application/json")
            .header("X-MLAI-Client", "laptop")
            .header("X-MLAI-Version", "0.3.4")
            .header("X-MLAI-Protocol", "1")
            .bearer_auth(token)
    }

    async fn parse<T: for<'de> Deserialize<'de>>(resp: reqwest::Response) -> Result<T> {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let message = parsed
                .get("error")
                .and_then(|e| e.get("message").or(Some(e)))
                .and_then(|e| e.as_str())
                .unwrap_or("ML AI Code request failed");
            return Err(anyhow!("{} ({})", message, status.as_u16()));
        }
        serde_json::from_str(&text).context("unreadable ML AI Code response")
    }

    pub async fn catalog(&self) -> Result<Catalog> {
        let resp = self
            .http
            .get(format!("{}/api/clients", self.server))
            .header("X-MLAI-Client", "laptop")
            .header("X-MLAI-Version", "0.3.4")
            .header("X-MLAI-Protocol", "1")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?;
        Self::parse(resp).await
    }

    pub async fn whoami(&self) -> Result<Value> {
        let resp = self
            .request("/api/laptop", &self.token)
            .json(&json!({"action": "whoami"}))
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await?;
        Self::parse(resp).await
    }

    pub async fn start_run(
        &self,
        project: &str,
        folder: &str,
        run_task: &str,
        model: &str,
    ) -> Result<StartResponse> {
        let resp = self
            .request("/api/laptop", &self.token)
            .json(&json!({
                "action": "start",
                "project": project,
                "folder": folder,
                "folderProject": project,
                "title": "ML AI Code bridge",
                "run": { "task": run_task, "model": model }
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;
        Self::parse(resp).await
    }

    pub async fn finish_run(&self, run_token: &str, state: &str, usage: Value) -> Result<()> {
        let resp = self
            .request("/api/laptop", run_token)
            .json(&json!({"action": "finish", "state": state, "usage": usage}))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;
        let _: Value = Self::parse(resp).await?;
        Ok(())
    }

    /// Upstream chat call; the caller decides how to read the body.
    pub async fn chat(
        &self,
        gateway: &str,
        body: &Value,
        run_token: &str,
        stream: bool,
    ) -> Result<reqwest::Response> {
        let base = gateway.trim_end_matches('/');
        let accept = if stream { "text/event-stream" } else { "application/json" };
        let resp = self
            .http
            .post(format!("{}/chat/completions", base))
            .header("Content-Type", "application/json")
            .header("Accept", accept)
            .bearer_auth(run_token)
            .json(body)
            .send()
            .await?;
        Ok(resp)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limits {
    pub context: i64,
    #[serde(default)]
    pub output: Option<i64>,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub seen: String,
}

#[derive(Clone, Debug)]
pub struct RunToken {
    pub model: String,
    pub token: String,
    pub gateway: String,
    pub expires: i64,
    pub run_id: Option<String>,
    pub budget: Option<i64>,
}

pub struct RunPool {
    client: Arc<MlaiClient>,
    project: String,
    folder: String,
    run_task: String,
    safety_ms: i64,
    limits_path: PathBuf,
    cache: Mutex<HashMap<String, RunToken>>,
    mint_lock: Mutex<()>,
    limits: Mutex<BTreeMap<String, Limits>>,
    /// Weighted units spent on the run currently cached for each model, since it was minted.
    consumed: StdMutex<HashMap<String, i64>>,
    /// Successful model calls on that run, reported on finish with the units.
    calls: StdMutex<HashMap<String, i64>>,
    /// Refusals and typed errors per model and code, for /usage.
    rejections: StdMutex<BTreeMap<String, BTreeMap<String, i64>>>,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// How much of a run's budget one response consumed, in the gateway's own units:
/// fresh input at 1x, cached input at 0.2x, output at 3x. These are the weights
/// the ML AI Code engine uses (lib/plans.ts, `billable_tokens`), so the count here
/// matches the number the server enforces.
pub fn weighted_units(usage: &Value) -> i64 {
    let num = |value: Option<&Value>| value.and_then(|v| v.as_i64()).unwrap_or(0).max(0);
    let read = num(usage.get("cache_read_input_tokens"));
    let prompt = match usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
        Some(prompt) => prompt.max(0),
        None => num(usage.get("input_tokens")) + read + num(usage.get("cache_creation_input_tokens")),
    };
    let output = match usage.get("completion_tokens").and_then(|v| v.as_i64()) {
        Some(output) => output.max(0),
        None => num(usage.get("output_tokens")),
    };
    if prompt == 0 && output == 0 {
        return num(usage.get("total_tokens"));
    }
    let cached = num(usage.pointer("/prompt_tokens_details/cached_tokens"))
        .max(num(usage.get("cached_tokens")))
        .max(read)
        .min(prompt);
    let fresh = prompt - cached;
    fresh + (cached * 2) / 10 + output * 3
}

/// Whether a run should be replaced before the next call. The reserve keeps one
/// more request's worth of budget (10%, capped at 4M) so a large prompt does not
/// cross the cap mid-flight. A run without a budget is not capped by the server.
pub fn needs_refresh(entry: &RunToken, used: i64) -> bool {
    match entry.budget {
        Some(budget) if budget > 0 => {
            let reserve = (budget / 10).min(4_000_000);
            used >= budget.saturating_sub(reserve)
        }
        _ => false,
    }
}

impl RunPool {
    pub fn new(client: Arc<MlaiClient>, cfg: &Config) -> Self {
        let limits_path = PathBuf::from(&cfg.limits_cache);
        let limits = std::fs::read_to_string(&limits_path)
            .ok()
            .and_then(|t| serde_json::from_str::<BTreeMap<String, Limits>>(&t).ok())
            .unwrap_or_default();
        RunPool {
            client,
            project: cfg.project.clone(),
            folder: cfg.folder.clone(),
            run_task: cfg.run_task.clone(),
            safety_ms: cfg.token_safety_ms as i64,
            limits_path,
            cache: Mutex::new(HashMap::new()),
            mint_lock: Mutex::new(()),
            limits: Mutex::new(limits),
            consumed: StdMutex::new(HashMap::new()),
            calls: StdMutex::new(HashMap::new()),
            rejections: StdMutex::new(BTreeMap::new()),
        }
    }

    pub async fn limits_for(&self, model: &str) -> Option<Limits> {
        self.limits.lock().await.get(model).cloned()
    }

    pub async fn limits_table(&self) -> BTreeMap<String, Limits> {
        self.limits.lock().await.clone()
    }

    /// Units already spent on the cached run for this model, since it was minted.
    pub fn consumed_of(&self, model: &str) -> i64 {
        self.consumed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(model)
            .copied()
            .unwrap_or(0)
    }

    pub fn calls_of(&self, model: &str) -> i64 {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(model)
            .copied()
            .unwrap_or(0)
    }

    /// Weighted units and successful calls for the run currently cached for this model.
    pub fn spend(&self, model: &str) -> (i64, i64) {
        (self.consumed_of(model), self.calls_of(model))
    }

    fn reset_spend(&self, model: &str) {
        self.consumed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(model);
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(model);
    }

    /// Records what a response cost, so the next call can replace the run before
    /// the gateway refuses it. Safe to call from sync code (the history writer).
    pub fn note_usage(&self, model: &str, units: i64) {
        *self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(model.to_string())
            .or_insert(0) += 1;
        if units <= 0 {
            return;
        }
        let mut consumed = self
            .consumed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *consumed.entry(model.to_string()).or_insert(0) += units;
    }

    /// Counts a refusal or typed error so /usage can show why a model was failing.
    pub fn note_rejection(&self, model: &str, code: &str) {
        let mut map = self
            .rejections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = map
            .entry(model.to_string())
            .or_default()
            .entry(code.to_string())
            .or_insert(0);
        *count += 1;
    }

    pub fn rejections(&self) -> BTreeMap<String, BTreeMap<String, i64>> {
        self.rejections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// The cached run, while it has not expired and still has budget left.
    async fn reusable(&self, model: &str) -> Option<RunToken> {
        let used = self.consumed_of(model);
        let cache = self.cache.lock().await;
        let hit = cache.get(model)?;
        if hit.expires - now_ms() > self.safety_ms && !needs_refresh(hit, used) {
            return Some(hit.clone());
        }
        None
    }

    pub async fn ensure(&self, model: &str) -> Result<RunToken> {
        if let Some(hit) = self.reusable(model).await {
            return Ok(hit);
        }
        let _guard = self.mint_lock.lock().await;
        if let Some(hit) = self.reusable(model).await {
            return Ok(hit);
        }
        let (spent, calls) = self.spend(model);
        let started = now_ms();
        let data = self
            .client
            .start_run(&self.project, &self.folder, &self.run_task, model)
            .await?;
        if data.intake.is_some() && data.token.is_none() {
            return Err(anyhow!("The server asked intake questions instead of starting a run. Set a neutral runTask in config."));
        }
        let token = data
            .token
            .clone()
            .ok_or_else(|| anyhow!("The server did not return a run token"))?;
        let entry = RunToken {
            model: model.to_string(),
            token,
            gateway: data
                .gateway
                .clone()
                .unwrap_or_else(|| DEFAULT_GATEWAY.to_string())
                .trim_end_matches('/')
                .to_string(),
            expires: data.expires.unwrap_or_else(|| now_ms() + 3_600_000),
            run_id: data.run_id.clone(),
            budget: data.budget,
        };
        let old = self.cache.lock().await.insert(model.to_string(), entry.clone());
        // A fresh run starts with its whole budget again.
        self.reset_spend(model);
        if let Some(old) = old {
            if old.token != entry.token {
                let client = self.client.clone();
                let usage = json!({"tokens": spent, "calls": calls});
                tokio::spawn(async move {
                    let _ = client.finish_run(&old.token, "completed", usage).await;
                });
            }
        }
        if let Some(model_info) = &data.model {
            if let Some(context) = model_info.context_window {
                let limits = Limits {
                    context,
                    output: model_info.response_reserve,
                    vision: model_info.vision.unwrap_or(true),
                    seen: chrono::Utc::now().to_rfc3339(),
                };
                let snapshot = {
                    let mut map = self.limits.lock().await;
                    map.insert(model.to_string(), limits);
                    map.clone()
                };
                if let Ok(text) = serde_json::to_string_pretty(&snapshot) {
                    let _ = std::fs::write(&self.limits_path, text);
                }
            }
        }
        tracing::info!(
            "minted run for {} in {}ms (expires {})",
            model,
            now_ms() - started,
            entry.expires
        );
        Ok(entry)
    }

    pub async fn invalidate(&self, model: &str) {
        let (spent, calls) = self.spend(model);
        self.reset_spend(model);
        if let Some(old) = self.cache.lock().await.remove(model) {
            let client = self.client.clone();
            let usage = json!({"tokens": spent, "calls": calls});
            tokio::spawn(async move {
                let _ = client.finish_run(&old.token, "completed", usage).await;
            });
        }
    }

    pub async fn finish_all(&self) {
        let entries: Vec<RunToken> = {
            let mut cache = self.cache.lock().await;
            let values = cache.values().cloned().collect();
            cache.clear();
            values
        };
        for entry in entries {
            let (spent, calls) = self.spend(&entry.model);
            let usage = json!({"tokens": spent, "calls": calls});
            let _ = self
                .client
                .finish_run(&entry.token, "completed", usage)
                .await;
        }
    }

    pub async fn snapshot(&self) -> Vec<Value> {
        self.cache
            .lock()
            .await
            .values()
            .map(|t| {
                json!({
                    "model": t.model, "runId": t.run_id, "expires": t.expires, "budget": t.budget,
                    "consumed": self.consumed_of(&t.model), "calls": self.calls_of(&t.model)
                })
            })
            .collect()
    }
}

pub fn credential_summary(cred: &Credential) -> Value {
    json!({
        "store": cred.store,
        "server": cred.server,
        "label": cred.label,
        "expires": cred.expires,
        "tokenPrefix": cred.token.chars().take(6).collect::<String>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(budget: Option<i64>) -> RunToken {
        RunToken {
            model: "pixis-opus".into(),
            token: "mlrun_test".into(),
            gateway: DEFAULT_GATEWAY.into(),
            expires: now_ms() + 3_600_000,
            run_id: None,
            budget,
        }
    }

    #[test]
    fn weights_match_the_engine() {
        // 1000 prompt, 800 of it cached, 100 out: 200 fresh + 160 cached + 300 output.
        let usage = json!({
            "prompt_tokens": 1000,
            "completion_tokens": 100,
            "prompt_tokens_details": {"cached_tokens": 800}
        });
        assert_eq!(weighted_units(&usage), 660);
    }

    #[test]
    fn cached_tokens_are_capped_at_the_prompt() {
        let usage = json!({"prompt_tokens": 10, "completion_tokens": 0, "cached_tokens": 99});
        // cached clamped to 10: 0 fresh + 2 cached + 0 output.
        assert_eq!(weighted_units(&usage), 2);
    }

    #[test]
    fn absent_usage_counts_nothing() {
        assert_eq!(weighted_units(&json!({})), 0);
    }

    #[test]
    fn uncapped_runs_never_refresh() {
        assert!(!needs_refresh(&run(None), i64::MAX));
    }

    #[test]
    fn refresh_reserves_the_last_tenth() {
        let entry = run(Some(40_000_000));
        assert!(!needs_refresh(&entry, 35_999_999));
        assert!(needs_refresh(&entry, 36_000_000));
    }

    #[test]
    fn small_budgets_reserve_only_a_tenth() {
        let entry = run(Some(1_000_000));
        assert!(!needs_refresh(&entry, 899_999));
        assert!(needs_refresh(&entry, 900_000));
    }

    #[tokio::test]
    async fn spend_and_calls_reset_when_a_run_is_dropped() {
        let client = Arc::new(MlaiClient::new("http://127.0.0.1:9", "test"));
        let pool = RunPool::new(client, &Config::default());
        pool.note_usage("m", 100);
        pool.note_usage("m", 50);
        assert_eq!(pool.spend("m"), (150, 2));
        pool.invalidate("m").await;
        assert_eq!(pool.spend("m"), (0, 0));
    }

    #[test]
    fn rejections_are_counted_per_model_and_code() {
        let client = Arc::new(MlaiClient::new("http://127.0.0.1:9", "test"));
        let pool = RunPool::new(client, &Config::default());
        pool.note_rejection("glm-5.3", "billing_limit");
        pool.note_rejection("glm-5.3", "billing_limit");
        pool.note_rejection("pixis-opus", "context_overflow");
        let counts = pool.rejections();
        assert_eq!(counts["glm-5.3"]["billing_limit"], 2);
        assert_eq!(counts["pixis-opus"]["context_overflow"], 1);
    }
}
