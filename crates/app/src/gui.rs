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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesktopSettings {
    pub autostart: bool,
    pub start_hidden: bool,
    pub close_to_tray: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        DesktopSettings { autostart: false, start_hidden: false, close_to_tray: false }
    }
}

fn read_desktop_settings(path: &PathBuf) -> DesktopSettings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_desktop_settings(path: &PathBuf, settings: &DesktopSettings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| e.to_string())
}

pub struct GuiState {
    config_path: PathBuf,
    config: Mutex<Config>,
    credential: Mutex<Option<Credential>>,
    proxy: tokio::sync::Mutex<Option<ProxyHandle>>,
    desktop_path: PathBuf,
    desktop: Mutex<DesktopSettings>,
}

impl GuiState {
    fn new(config_path: PathBuf, config: Config) -> Self {
        let desktop_path = config_path.with_file_name("desktop.json");
        let desktop = read_desktop_settings(&desktop_path);
        GuiState {
            config_path,
            config: Mutex::new(config),
            credential: Mutex::new(None),
            proxy: tokio::sync::Mutex::new(None),
            desktop_path,
            desktop: Mutex::new(desktop),
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

    fn desktop_settings_value(&self) -> Value {
        json!(self.desktop.lock().unwrap().clone())
    }

    fn set_desktop_value(&self, settings: Value) -> Result<Value, String> {
        let mut current = self.desktop.lock().unwrap().clone();
        if let Some(v) = settings.get("autostart").and_then(|v| v.as_bool()) {
            current.autostart = v;
        }
        if let Some(v) = settings.get("startHidden").and_then(|v| v.as_bool()) {
            current.start_hidden = v;
        }
        if let Some(v) = settings.get("closeToTray").and_then(|v| v.as_bool()) {
            current.close_to_tray = v;
        }
        write_desktop_settings(&self.desktop_path, &current)?;
        *self.desktop.lock().unwrap() = current.clone();
        Ok(json!(current))
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
                    "expires": account.expires,
                    "lastCheckAt": account.last_check_at,
                    "lastCheckOk": account.last_check_ok,
                    "lastError": account.last_error,
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

    /// Verify each account's device token with a read-only whoami call and store the outcome.
    async fn accounts_check_value(&self, id: Option<String>) -> Result<Value, String> {
        let accounts = self.accounts()?;
        let file = accounts.file();
        let targets = match id {
            Some(id) => vec![file.find(&id).cloned().ok_or_else(|| "no such account".to_string())?],
            None => file.accounts.clone(),
        };
        let mut out = Vec::new();
        for account in targets {
            let started = std::time::Instant::now();
            let result = match accounts.token(&account.id) {
                Ok(token) => mlai_bridge_core::mlai::MlaiClient::new(&account.server, &token).whoami().await,
                Err(error) => Err(error),
            };
            let ms = started.elapsed().as_millis() as i64;
            let error = result.as_ref().err().map(|e| format!("{:#}", e));
            let ok = result.is_ok();
            let _ = accounts.record_check(&account.id, ok, error.clone());
            out.push(json!({"id": account.id, "ok": ok, "ms": ms, "error": error}));
        }
        Ok(json!(out))
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
            .add_full(&label, credential.server.trim_end_matches('/'), &credential.token, credential.expires)
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

    /// One tiny real request through the local proxy, so the check exercises the same path as clients.
    async fn test_connection_value(&self, requested: Option<String>) -> Result<Value, String> {
        let (base, key, model) = {
            let proxy = self.proxy.lock().await;
            let Some(handle) = proxy.as_ref() else {
                return Err("The proxy is not running. Start it first.".into());
            };
            let config = handle.state.config.clone();
            let model = match requested {
                Some(model) if !model.trim().is_empty() => model,
                _ if config.aliases.contains_key(&config.default_model) => config.default_model.clone(),
                _ => config.aliases.keys().next().cloned().unwrap_or(config.default_model.clone()),
            };
            (handle.base_url(), config.api_key.clone(), model)
        };
        let body = json!({
            "model": model,
            "messages": [{"role": "user", "content": "Reply with the single word: ok"}],
            "max_tokens": 16,
            "stream": false
        });
        let started = std::time::Instant::now();
        let sent = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", base))
            .bearer_auth(&key)
            .json(&body)
            .timeout(std::time::Duration::from_secs(45))
            .send()
            .await;
        let ms = started.elapsed().as_millis() as i64;
        match sent {
            Ok(response) => {
                let status = response.status().as_u16();
                let ok = response.status().is_success();
                let text = response.text().await.unwrap_or_default();
                let snippet: String = text.chars().take(400).collect();
                Ok(json!({"ok": ok, "ms": ms, "status": status, "model": model, "body": snippet}))
            }
            Err(error) => Ok(json!({"ok": false, "ms": ms, "status": 0, "model": model, "error": format!("{:#}", error)})),
        }
    }

    async fn history_value(&self, limit: Option<i64>) -> Result<Vec<RequestRow>, String> {
        let config = self.config.lock().unwrap().clone();
        let store = Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes).map_err(|e| e.to_string())?;
        store
            .recent(limit.unwrap_or(50).clamp(1, 5000))
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

    async fn usage_window_value(&self, from_ms: i64, to_ms: i64, buckets: i64) -> Result<Value, String> {
        let config = self.config.lock().unwrap().clone();
        let store = Store::open(std::path::Path::new(&config.history_path), config.body_keep, config.body_max_bytes).map_err(|e| e.to_string())?;
        let mut value = store.window(from_ms, to_ms, buckets).map_err(|e| e.to_string())?;
        if let Some(map) = value.as_object_mut() {
            map.insert("tokensPerCredit".into(), json!(mlai_bridge_core::store::TOKENS_PER_CREDIT));
        }
        Ok(value)
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
async fn test_connection(state: State<'_, Arc<GuiState>>, model: Option<String>) -> Result<Value, String> {
    state.test_connection_value(model).await
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
#[tauri::command]
async fn usage_window(state: State<'_, Arc<GuiState>>, from_ms: i64, to_ms: i64, buckets: i64) -> Result<Value, String> {
    state.usage_window_value(from_ms, to_ms, buckets).await
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
async fn accounts_check(state: State<'_, Arc<GuiState>>, id: Option<String>) -> Result<Value, String> {
    state.accounts_check_value(id).await
}

#[tauri::command]
fn desktop_settings(state: State<'_, Arc<GuiState>>) -> Value {
    state.desktop_settings_value()
}

#[tauri::command]
fn set_desktop_settings(app: tauri::AppHandle, state: State<'_, Arc<GuiState>>, settings: Value) -> Result<Value, String> {
    let before = state.desktop.lock().unwrap().autostart;
    let value = state.set_desktop_value(settings)?;
    let after = value.get("autostart").and_then(|v| v.as_bool()).unwrap_or(false);
    if after != before {
        use tauri_plugin_autostart::ManagerExt;
        let manager = app.autolaunch();
        if after {
            manager.enable().map_err(|e| e.to_string())?;
        } else {
            manager.disable().map_err(|e| e.to_string())?;
        }
    }
    Ok(value)
}

#[tauri::command]
fn free_port(after: u16) -> Result<u16, String> {
    for offset in 1..=50u32 {
        let port = after as u32 + offset;
        if port > 65535 {
            break;
        }
        if std::net::TcpListener::bind(("127.0.0.1", port as u16)).is_ok() {
            return Ok(port as u16);
        }
    }
    Err("No free port in the next 50 ports.".into())
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
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let (config, _exists) = Config::load(&config_path)?;
    let state = Arc::new(GuiState::new(config_path, config));
    let hidden_on_launch = state.desktop.lock().unwrap().start_hidden || std::env::args().any(|a| a == "--minimized");

    tauri::Builder::default()
        // Must be the first plugin so a second launch forwards to this instance instead of opening a new window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(state)
        .setup(move |app| {
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

            let show_item = MenuItem::with_id(app, "show", "Open MLAI Bridge", true, None::<&str>)?;
            let start_item = MenuItem::with_id(app, "start", "Start proxy", true, None::<&str>)?;
            let stop_item = MenuItem::with_id(app, "stop", "Stop proxy", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &start_item, &stop_item, &quit_item])?;
            TrayIconBuilder::with_id("mlai-tray")
                .tooltip("MLAI Bridge")
                .icon(app.default_window_icon().cloned().ok_or_else(|| anyhow::anyhow!("no window icon"))?)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "start" | "stop" => {
                        let gui = app.state::<Arc<GuiState>>().inner().clone();
                        let start = event.id().as_ref() == "start";
                        tauri::async_runtime::spawn(async move {
                            if start {
                                let _ = gui.start_value().await;
                            } else {
                                let _ = gui.stop_value().await;
                            }
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            tracing::info!("tray icon created");

            let data_dir = writable_data_dir();
            tracing::info!("webview data dir: {}", data_dir.display());
            let window = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                .title("MLAI Bridge")
                .inner_size(1120.0, 760.0)
                .min_inner_size(900.0, 600.0)
                .center()
                .data_directory(data_dir)
                .background_color(tauri::window::Color(24, 24, 27, 255))
                .visible(!hidden_on_launch)
                .build()?;
            {
                let gui_for_close = gui.clone();
                let window_for_close = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if gui_for_close.desktop.lock().unwrap().close_to_tray {
                            api.prevent_close();
                            let _ = window_for_close.hide();
                        }
                    }
                });
            }
            if !hidden_on_launch {
                let _ = window.show();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, save_config, start, stop, test_connection, history, clear_history, models, mint_limits, health, usage, usage_window, request_detail,
            accounts, accounts_add, accounts_activate, accounts_remove, accounts_import, accounts_check,
            desktop_settings, set_desktop_settings, free_port, startup_tab
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
