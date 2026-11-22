//! Tauri control panel plus a headless self-test of the same code path.
use mlai_bridge_core::accounts::Accounts;
use mlai_bridge_core::auth::{self, Credential};
use mlai_bridge_core::config::Config;
use mlai_bridge_core::store::{RequestRow, Store};
use mlai_bridge_core::{start_proxy, ProxyHandle};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

pub struct GuiState {
    config_path: PathBuf,
    config: Mutex<Config>,
    credential: Mutex<Option<Credential>>,
    proxy: tokio::sync::Mutex<Option<ProxyHandle>>,
}

impl GuiState {
    fn new(config_path: PathBuf, config: Config) -> Self {
        GuiState {
            config_path,
            config: Mutex::new(config),
            credential: Mutex::new(None),
            proxy: tokio::sync::Mutex::new(None),
        }
    }

    fn running_bits(handle: &ProxyHandle) -> Value {
        let base = handle.base_url();
        json!({
            "running": true,
            "baseUrl": base,
            "v1": format!("{}/v1", base),
            "key": handle.state.config.api_key,
            "addr": handle.addr.to_string(),
        })
    }

    async fn models_value(&self) -> Result<Value, String> {
        let proxy = self.proxy.lock().await;
        let Some(handle) = proxy.as_ref() else { return Ok(json!([])) };
        let limits = handle.state.pool.limits_table().await;
        let config = handle.state.config.clone();
        let mut out = Vec::new();
        for (alias, real) in config.aliases.iter() {
            let limit = limits.get(real);
            out.push(json!({
                "id": alias, "real": real, "owned_by": "mlai",
                "context": limit.map(|l| l.context),
                "output": limit.and_then(|l| l.output),
                "vision": limit.map(|l| l.vision),
            }));
        }
        for id in handle.state.catalog_ids.iter() {
            if config.aliases.values().any(|v| v == id) {
                continue;
            }
            let limit = limits.get(id);
            out.push(json!({
                "id": id, "real": id, "owned_by": "mlai",
                "context": limit.map(|l| l.context),
                "output": limit.and_then(|l| l.output),
                "vision": limit.map(|l| l.vision),
            }));
        }
        Ok(Value::Array(out))
    }

    fn stats_value(&self) -> Option<mlai_bridge_core::store::Stats> {
        let config = self.config.lock().unwrap().clone();
        Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes)
            .ok()
            .and_then(|store| store.stats().ok())
    }

    async fn bootstrap_value(&self) -> Result<Value, String> {
        let path = self.config_path.clone();
        let (config, exists) = Config::load(&path).map_err(|e| e.to_string())?;
        *self.config.lock().unwrap() = config.clone();
        let credential = auth::load(&config.server).ok();
        *self.credential.lock().unwrap() = credential.clone();
        let running = {
            let proxy = self.proxy.lock().await;
            match proxy.as_ref() {
                Some(handle) => Self::running_bits(handle),
                None => json!({"running": false}),
            }
        };
        let accounts = self.accounts_value().await.unwrap_or_else(|_| json!({"accounts": [], "active": null}));
        let models = self.models_value().await.unwrap_or_else(|_| json!([]));
        Ok(json!({
            "configPath": path.to_string_lossy(),
            "configExists": exists,
            "config": config,
            "credential": credential.as_ref().map(|c| json!({
                "store": c.store, "server": c.server, "label": c.label,
                "tokenPrefix": c.token.chars().take(6).collect::<String>(),
            })),
            "running": running,
            "models": models,
            "stats": self.stats_value(),
            "accounts": accounts,
        }))
    }

    async fn save_value(&self, config: Value) -> Result<Value, String> {
        let mut parsed: Config = serde_json::from_value(config).map_err(|e| e.to_string())?;
        parsed.validate()?;
        parsed.resolve_paths();
        let path = self.config_path.clone();
        parsed.save(&path).map_err(|e| e.to_string())?;
        *self.config.lock().unwrap() = parsed;
        let running = self.proxy.lock().await.is_some();
        Ok(json!({"ok": true, "restartRequired": running}))
    }

    fn accounts(&self) -> Result<Accounts, String> {
        let config = self.config.lock().unwrap().clone();
        Accounts::open(std::path::Path::new(&config.accounts_path)).map_err(|e| e.to_string())
    }

    /// Active account's credential, falling back to the mlai CLI sign-in on this machine.
    fn resolve_active(&self) -> Result<(Credential, String, String), String> {
        let config = self.config.lock().unwrap().clone();
        let fallback = auth::load(&config.server).unwrap_or(Credential {
            token: String::new(),
            server: config.server.clone(),
            store: "none".into(),
            label: None,
            expires: None,
        });
        let accounts = self.accounts()?;
        let (credential, id, label) = accounts.active_credential(&fallback).map_err(|e| e.to_string())?;
        if credential.token.is_empty() {
            return Err("No active account. Add one on the Accounts tab, or run mlai login.".into());
        }
        Ok((credential, id, label))
    }

    async fn accounts_value(&self) -> Result<Value, String> {
        let accounts = self.accounts()?;
        let file = accounts.file();
        let list: Vec<Value> = file
            .accounts
            .iter()
            .map(|account| {
                json!({
                    "id": account.id,
                    "label": account.label,
                    "server": account.server,
                    "tokenHint": account.token_hint,
                    "createdAt": account.created_at,
                    "lastUsed": account.last_used,
                    "active": file.active.as_deref() == Some(account.id.as_str()),
                })
            })
            .collect();
        Ok(json!({
            "accounts": list,
            "active": file.active,
            "path": self.config.lock().unwrap().accounts_path,
        }))
    }

    async fn accounts_add_value(&self, token: String, label: Option<String>, server: Option<String>) -> Result<Value, String> {
        let config = self.config.lock().unwrap().clone();
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err("Paste the account's mlai device token".into());
        }
        let server = server.filter(|s| !s.trim().is_empty()).unwrap_or(config.server.clone());
        let client = mlai_bridge_core::mlai::MlaiClient::new(&server, &token);
        let who = client
            .whoami()
            .await
            .map_err(|e| format!("That token was rejected: {:#}", e))?;
        let fallback_label = who
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| who.get("email").and_then(|v| v.as_str()))
            .or_else(|| who.get("label").and_then(|v| v.as_str()))
            .unwrap_or("ML AI Code account")
            .to_string();
        let label = label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).unwrap_or(fallback_label);
        let accounts = self.accounts()?;
        let first = accounts.file().accounts.is_empty();
        accounts
            .add(&label, server.trim_end_matches('/'), &token)
            .map_err(|e| e.to_string())?;
        if first {
            self.restart_if_running().await?;
        }
        self.accounts_value().await
    }

    /// Stop and start the proxy so new credential/account takes effect.
    async fn restart_if_running(&self) -> Result<Option<Value>, String> {
        let handle = self.proxy.lock().await.take();
        match handle {
            Some(handle) => {
                handle.stop().await;
                let started = self.start_value().await?;
                Ok(Some(started))
            }
            None => Ok(None),
        }
    }

    async fn accounts_activate_value(&self, id: String, restart: bool) -> Result<Value, String> {
        let accounts = self.accounts()?;
        accounts.activate(&id).map_err(|e| e.to_string())?;
        if restart {
            let started = self.restart_if_running().await?;
            return Ok(json!({"ok": true, "restarted": started.is_some(), "running": started}));
        }
        self.accounts_value().await
    }

    async fn accounts_remove_value(&self, id: String) -> Result<Value, String> {
        let accounts = self.accounts()?;
        accounts.remove(&id).map_err(|e| e.to_string())?;
        self.accounts_value().await
    }

    async fn accounts_import_value(&self) -> Result<Value, String> {
        let config = self.config.lock().unwrap().clone();
        let credential = auth::load(&config.server).map_err(|e| e.to_string())?;
        let client = mlai_bridge_core::mlai::MlaiClient::new(&credential.server, &credential.token);
        let who = client.whoami().await.map_err(|e| format!("{:#}", e))?;
        let label = who
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| who.get("email").and_then(|v| v.as_str()))
            .unwrap_or("mlai CLI account")
            .to_string();
        let accounts = self.accounts()?;
        let first = accounts.file().accounts.is_empty();
        accounts
            .add(&label, credential.server.trim_end_matches('/'), &credential.token)
            .map_err(|e| e.to_string())?;
        if first {
            self.restart_if_running().await?;
        }
        self.accounts_value().await
    }

    async fn start_value(&self) -> Result<Value, String> {
        {
            let proxy = self.proxy.lock().await;
            if let Some(handle) = proxy.as_ref() {
                return Ok(Self::running_bits(handle));
            }
        }
        let config = self.config.lock().unwrap().clone();
        let (credential, account_id, account_label) = self.resolve_active()?;
        *self.credential.lock().unwrap() = Some(credential.clone());
        let handle = start_proxy(config, credential, account_id, account_label)
            .await
            .map_err(|e| format!("{:#}", e))?;
        let bits = Self::running_bits(&handle);
        *self.proxy.lock().await = Some(handle);
        Ok(bits)
    }

    async fn stop_value(&self) -> Result<Value, String> {
        let handle = self.proxy.lock().await.take();
        if let Some(handle) = handle {
            handle.stop().await;
        }
        Ok(json!({"running": false}))
    }

    async fn history_value(&self, limit: Option<i64>) -> Result<Vec<RequestRow>, String> {
        let config = self.config.lock().unwrap().clone();
        let store = Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes).map_err(|e| e.to_string())?;
        store
            .recent(limit.unwrap_or(50).clamp(1, 500))
            .map_err(|e| e.to_string())
    }

    async fn clear_value(&self) -> Result<Value, String> {
        let config = self.config.lock().unwrap().clone();
        let store = Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes).map_err(|e| e.to_string())?;
        store.clear().map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    }

    async fn usage_value(&self) -> Result<Value, String> {
        let config = self.config.lock().unwrap().clone();
        let store = Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes).map_err(|e| e.to_string())?;
        Ok(json!({
            "stats": store.stats().ok(),
            "byModel": store.by_model().unwrap_or_default(),
            "byAccount": store.by_account().unwrap_or_default(),
            "tokensPerCredit": mlai_bridge_core::store::TOKENS_PER_CREDIT,
        }))
    }

    async fn request_detail_value(&self, id: i64) -> Result<Value, String> {
        let config = self.config.lock().unwrap().clone();
        let store = Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes)
            .map_err(|e| e.to_string())?;
        let row = store
            .get(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Request {} is not in the recent history", id))?;
        let body = store.load_body(id).map_err(|e| e.to_string())?;
        Ok(json!({
            "row": row,
            "hasBody": body.is_some(),
            "request": body.as_ref().and_then(|b| b.get("request")),
            "response": body.as_ref().and_then(|b| b.get("response")),
        }))
    }

    async fn mint_limits_value(&self) -> Result<Value, String> {
        let proxy = self.proxy.lock().await;
        let Some(handle) = proxy.as_ref() else { return Err("Start the proxy first".into()) };
        let mut targets: Vec<String> = Vec::new();
        for real in handle.state.config.aliases.values() {
            if !targets.contains(real) {
                targets.push(real.clone());
            }
        }
        let mut learned = Vec::new();
        for model in targets {
            match handle.state.pool.ensure(&model).await {
                Ok(_) => {
                    if let Some(limit) = handle.state.pool.limits_for(&model).await {
                        learned.push(json!({"model": model, "context": limit.context, "output": limit.output}));
                    }
                }
                Err(error) => learned.push(json!({"model": model, "error": error.to_string()})),
            }
        }
        Ok(Value::Array(learned))
    }
}

#[tauri::command]
async fn bootstrap(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.bootstrap_value().await
}
#[tauri::command]
async fn save_config(state: State<'_, Arc<GuiState>>, config: Value) -> Result<Value, String> {
    state.save_value(config).await
}
#[tauri::command]
async fn start(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.start_value().await
}
#[tauri::command]
async fn stop(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.stop_value().await
}
#[tauri::command]
async fn history(state: State<'_, Arc<GuiState>>, limit: Option<i64>) -> Result<Vec<RequestRow>, String> {
    state.history_value(limit).await
}
#[tauri::command]
async fn clear_history(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.clear_value().await
}
#[tauri::command]
async fn models(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.models_value().await
}
#[tauri::command]
async fn mint_limits(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.mint_limits_value().await
}
#[tauri::command]
async fn usage(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.usage_value().await
}
/// Which tab to open first (MLAI_BRIDGE_TAB=usage|proxy), for screenshots and demos.
#[tauri::command]
async fn request_detail(state: State<'_, Arc<GuiState>>, id: i64) -> Result<Value, String> {
    state.request_detail_value(id).await
}
#[tauri::command]
async fn accounts(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.accounts_value().await
}
#[tauri::command]
async fn accounts_add(
    state: State<'_, Arc<GuiState>>,
    token: String,
    label: Option<String>,
    server: Option<String>,
) -> Result<Value, String> {
    state.accounts_add_value(token, label, server).await
}
#[tauri::command]
async fn accounts_activate(
    state: State<'_, Arc<GuiState>>,
    id: String,
    restart: Option<bool>,
) -> Result<Value, String> {
    state.accounts_activate_value(id, restart.unwrap_or(false)).await
}
#[tauri::command]
async fn accounts_remove(state: State<'_, Arc<GuiState>>, id: String) -> Result<Value, String> {
    state.accounts_remove_value(id).await
}
#[tauri::command]
async fn accounts_import(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    state.accounts_import_value().await
}

#[tauri::command]
fn startup_tab() -> String {
    std::env::var("MLAI_BRIDGE_TAB").unwrap_or_default()
}
#[tauri::command]
async fn health(state: State<'_, Arc<GuiState>>) -> Result<Value, String> {
    let proxy = state.proxy.lock().await;
    let Some(handle) = proxy.as_ref() else { return Ok(json!({"running": false})) };
    Ok(json!({
        "running": true,
        "baseUrl": handle.base_url(),
        "runs": handle.state.pool.snapshot().await,
    }))
}

fn writable_data_dir() -> PathBuf {
    let candidates = [
        std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("mlai-bridge").join("webview")),
        Some(mlai_bridge_core::config::home_dir().join("webview")),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(".webview"))),
        std::env::current_dir().ok().map(|d| d.join(".webview")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if std::fs::create_dir_all(&candidate).is_err() {
            continue;
        }
        let probe = candidate.join(".probe");
        if std::fs::write(&probe, b"1").is_ok() {
            let _ = std::fs::remove_file(&probe);
            return candidate;
        }
    }
    std::env::temp_dir().join("mlai-bridge-webview")
}

pub fn run(config_path: PathBuf) -> anyhow::Result<()> {
    let (config, _exists) = Config::load(&config_path)?;
    let state = Arc::new(GuiState::new(config_path, config));
    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            let gui = app.state::<Arc<GuiState>>().inner().clone();
            tauri::async_runtime::spawn({
                let gui = gui.clone();
                async move {
                    let config = gui.config.lock().unwrap().clone();
                    if config.project.is_empty() {
                        return;
                    }
                    let Ok((credential, account_id, account_label)) = gui.resolve_active() else { return };
                    if let Ok(handle) = start_proxy(config, credential, account_id, account_label).await {
                        *gui.proxy.lock().await = Some(handle);
                    }
                }
            });
            let data_dir = writable_data_dir();
            tracing::info!("webview data dir: {}", data_dir.display());
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                .title("MLAI Bridge")
                .inner_size(1120.0, 760.0)
                .min_inner_size(900.0, 600.0)
                .center()
                .data_directory(data_dir)
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, save_config, start, stop, history, clear_history, models, mint_limits, health, usage, request_detail,
            accounts, accounts_add, accounts_activate, accounts_remove, accounts_import, startup_tab
        ])
        .run(tauri::generate_context!())
        .map_err(|e| anyhow::anyhow!(e))
}

/// Drive the exact code path the buttons use, without opening a window.
pub fn self_test(config_path: PathBuf) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let (config, _exists) = Config::load(&config_path)?;
        let state = Arc::new(GuiState::new(config_path, config));
        let boot = state.bootstrap_value().await.map_err(anyhow::Error::msg)?;
        println!(
            "bootstrap: config={} credential={} aliases={}",
            boot.get("configExists").and_then(|v| v.as_bool()).unwrap_or(false),
            boot.get("credential").map(|v| !v.is_null()).unwrap_or(false),
            boot.get("config")
                .and_then(|c| c.get("aliases"))
                .and_then(|a| a.as_object())
                .map(|a| a.len())
                .unwrap_or(0)
        );
        let started = state.start_value().await.map_err(anyhow::Error::msg)?;
        let v1 = started.get("v1").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        println!("start: {}", v1);
        let key = state.config.lock().unwrap().api_key.clone();
        let models = state.models_value().await.map_err(anyhow::Error::msg)?;
        let alias = models
            .as_array()
            .and_then(|list| list.first())
            .and_then(|m| m.get("id"))
            .and_then(|id| id.as_str())
            .unwrap_or("mlai-code")
            .to_string();
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/chat/completions", v1))
            .bearer_auth(&key)
            .json(&json!({
                "model": alias,
                "messages": [{"role": "user", "content": "Reply with exactly: GUI-OK"}],
                "max_tokens": 64,
                "temperature": 0
            }))
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        let content = body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();
        println!("chat: status={} model={} content={:?}", status.as_u16(), alias, content);
        if !status.is_success() || content.is_empty() {
            anyhow::bail!("chat through the GUI-owned proxy failed");
        }
        let rows = state.history_value(Some(10)).await.map_err(anyhow::Error::msg)?;
        println!("history rows: {}", rows.len());
        if rows.is_empty() {
            anyhow::bail!("history did not record the request");
        }
        state.stop_value().await.map_err(anyhow::Error::msg)?;
        println!("stop: ok");
        println!("self-test passed");
        Ok(())
    })
}
