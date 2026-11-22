//! ML AI Code control plane: run tokens (one per model), catalog, finish.
use crate::auth::Credential;
use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
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
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
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
        }
    }

    pub async fn limits_for(&self, model: &str) -> Option<Limits> {
        self.limits.lock().await.get(model).cloned()
    }

    pub async fn limits_table(&self) -> BTreeMap<String, Limits> {
        self.limits.lock().await.clone()
    }

    pub async fn ensure(&self, model: &str) -> Result<RunToken> {
        {
            let cache = self.cache.lock().await;
            if let Some(hit) = cache.get(model) {
                if hit.expires - now_ms() > self.safety_ms {
                    return Ok(hit.clone());
                }
            }
        }
        let _guard = self.mint_lock.lock().await;
        {
            let cache = self.cache.lock().await;
            if let Some(hit) = cache.get(model) {
                if hit.expires - now_ms() > self.safety_ms {
                    return Ok(hit.clone());
                }
            }
        }
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
        if let Some(old) = old {
            if old.token != entry.token {
                let client = self.client.clone();
                tokio::spawn(async move {
                    let _ = client.finish_run(&old.token, "completed", json!({})).await;
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
        if let Some(old) = self.cache.lock().await.remove(model) {
            let client = self.client.clone();
            tokio::spawn(async move {
                let _ = client.finish_run(&old.token, "completed", json!({})).await;
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
            let _ = self
                .client
                .finish_run(&entry.token, "completed", json!({}))
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
                    "model": t.model, "runId": t.run_id, "expires": t.expires, "budget": t.budget
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
