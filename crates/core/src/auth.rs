//! Reuse the device credential the mlai CLI already stored (~/.mlai).
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Credential {
    pub token: String,
    pub server: String,
    pub store: String,
    pub label: Option<String>,
    pub expires: Option<i64>,
}

#[derive(Deserialize)]
struct AuthMeta {
    server: Option<String>,
    store: Option<String>,
    label: Option<String>,
    expires: Option<i64>,
}

pub fn mlai_home() -> PathBuf {
    if let Ok(dir) = std::env::var("MLAI_HOME") {
        return PathBuf::from(dir);
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".mlai")
}

pub fn load(server: &str) -> anyhow::Result<Credential> {
    if let Ok(token) = std::env::var("MLAI_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() {
            let server = std::env::var("MLAI_SERVER").unwrap_or_else(|_| server.to_string());
            return Ok(Credential { token, server, store: "env".into(), label: None, expires: None });
        }
    }
    let home = mlai_home();
    let meta_raw = std::fs::read_to_string(home.join("auth.json")).unwrap_or_default();
    let meta: AuthMeta = serde_json::from_str(&meta_raw).unwrap_or(AuthMeta {
        server: None,
        store: None,
        label: None,
        expires: None,
    });
    let token = std::fs::read_to_string(home.join(".device-token"))
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    if token.is_empty() {
        anyhow::bail!("No ML AI Code sign-in found. Run mlai login (or set MLAI_TOKEN) first.");
    }
    if let Some(exp) = meta.expires {
        if exp < chrono::Utc::now().timestamp_millis() {
            anyhow::bail!("The ML AI Code sign-in on this machine expired. Run mlai login again.");
        }
    }
    Ok(Credential {
        token,
        server: meta.server.unwrap_or_else(|| server.to_string()),
        store: meta.store.unwrap_or_else(|| "file".into()),
        label: meta.label,
        expires: meta.expires,
    })
}
