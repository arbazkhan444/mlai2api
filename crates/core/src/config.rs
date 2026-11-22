//! config.json shared with the Node bridge: same field names, same meaning.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningMode {
    Native,
    Reasoning,
    Both,
    None,
}

impl Default for ReasoningMode {
    fn default() -> Self {
        ReasoningMode::Native
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub server: String,
    pub api_key: String,
    pub project: String,
    pub folder: String,
    pub run_task: String,
    pub default_model: String,
    pub aliases: BTreeMap<String, String>,
    pub reasoning: ReasoningMode,
    pub reasoning_effort: Option<String>,
    pub always_include_usage: bool,
    pub allow_raw_model_ids: bool,
    pub token_safety_ms: u64,
    pub request_timeout_ms: u64,
    pub max_tokens_floor: u64,
    pub limits_cache: String,
    pub history_path: String,
    pub body_keep: i64,
    pub body_max_bytes: i64,
    pub accounts_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "127.0.0.1".into(),
            port: 8787,
            server: "https://code.ml.ai".into(),
            api_key: String::new(),
            project: String::new(),
            folder: String::new(),
            run_task: "Local model bridge session".into(),
            default_model: "mlai-code".into(),
            aliases: BTreeMap::new(),
            reasoning: ReasoningMode::Native,
            reasoning_effort: Some("low".into()),
            always_include_usage: true,
            allow_raw_model_ids: true,
            token_safety_ms: 300_000,
            request_timeout_ms: 900_000,
            max_tokens_floor: 0,
            limits_cache: String::new(),
            history_path: String::new(),
            body_keep: 200,
            body_max_bytes: 64 * 1024 * 1024,
            accounts_path: String::new(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("port must be 1-65535".into());
        }
        if let Some(effort) = &self.reasoning_effort {
            if !["low", "medium", "high", "max"].contains(&effort.as_str()) {
                return Err("reasoningEffort must be low, medium, high, max or null".into());
            }
        }
        Ok(())
    }

    pub fn resolve_paths(&mut self) {
        self.history_path = ensure_writable_file(&self.history_path, "requests.jsonl");
        self.limits_cache = ensure_writable_file(&self.limits_cache, "limits.cache.json");
        if self.accounts_path.trim().is_empty() {
            let dir = std::path::Path::new(&self.history_path)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(home_dir);
            self.accounts_path = dir.join("accounts.json").to_string_lossy().to_string();
        }
    }

    pub fn load(path: &Path) -> anyhow::Result<(Config, bool)> {
        if !path.exists() {
            let mut cfg = Config::default();
            cfg.resolve_paths();
            return Ok((cfg, false));
        }
        let text = fs::read_to_string(path)?;
        let mut cfg: Config = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("config.json is not valid: {}", e))?;
        cfg.validate().map_err(|e| anyhow::anyhow!(e))?;
        cfg.resolve_paths();
        Ok((cfg, true))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)? + "\n";
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}

pub fn home_dir() -> PathBuf {
    let dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".mlai-bridge");
    let _ = fs::create_dir_all(&dir);
    dir
}

use std::io::Write as _IoWrite;

fn try_append(path: &std::path::Path) -> bool {
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => file.write_all(b"").is_ok(),
        Err(_) => false,
    }
}

/// The first location this process can actually write to wins: the configured path,
/// %LOCALAPPDATA%\mlai-bridge, ~/.mlai-bridge, %TEMP%\mlai-bridge, then the cwd.
pub fn ensure_writable_file(preferred: &str, name: &str) -> String {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if !preferred.trim().is_empty() {
        candidates.push(std::path::PathBuf::from(preferred));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(std::path::PathBuf::from(local).join("mlai-bridge").join(name));
    }
    candidates.push(home_dir().join(name));
    if let Some(tmp) = std::env::var_os("TEMP") {
        candidates.push(std::path::PathBuf::from(tmp).join("mlai-bridge").join(name));
    }
    candidates.push(std::path::PathBuf::from(name));
    for candidate in candidates.iter() {
        if try_append(candidate) {
            return candidate.to_string_lossy().to_string();
        }
    }
    preferred.to_string()
}

/// Where config.json lives when no path is given.
pub fn default_config_path() -> PathBuf {
    if let Ok(p) = std::env::var("MLAI_BRIDGE_CONFIG") {
        return PathBuf::from(p);
    }
    let local = std::env::var_os("LOCALAPPDATA")
        .map(|dir| PathBuf::from(dir).join("mlai-bridge").join("config.json"));
    if let Some(path) = local.as_ref() {
        if path.exists() {
            return path.clone();
        }
    }
    let user = home_dir().join("config.json");
    if user.exists() {
        return user;
    }
    local.unwrap_or(user)
}

pub fn new_api_key() -> String {
    let mut bytes = [0u8; 24];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("mlba_{}", hex::encode(bytes))
}
