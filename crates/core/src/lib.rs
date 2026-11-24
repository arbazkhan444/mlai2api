//! mlai-bridge-core: the proxy, the run-token pool, config, credentials and history.
pub mod accounts;
pub mod anthropic;
pub mod auth;
pub mod capability;
pub mod config;
pub mod context;
pub mod mlai;
pub mod server;
pub mod store;
pub mod transform;

use anyhow::{Context, Result};
use config::Config;
use mlai::{MlaiClient, RunPool};
use server::AppState;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use store::Store;

pub struct ProxyHandle {
    pub addr: SocketAddr,
    pub state: AppState,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl ProxyHandle {
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
        self.state.pool.finish_all().await;
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.addr.ip(), self.addr.port())
    }
}

pub async fn start_proxy(
    config: Config,
    credential: auth::Credential,
    account_id: String,
    account_label: String,
) -> Result<ProxyHandle> {
    config.validate().map_err(|e| anyhow::anyhow!(e))?;
    let client = Arc::new(MlaiClient::new(&config.server, &credential.token));
    let catalog = client.catalog().await.unwrap_or_default();
    let catalog_ids: Vec<String> = catalog
        .models
        .iter()
        .filter(|m| m.available.unwrap_or(true))
        .map(|m| m.id.clone())
        .collect();
    let catalog_labels: Vec<(String, String)> = catalog
        .models
        .iter()
        .map(|m| (m.id.clone(), m.label.clone().unwrap_or_else(|| m.id.clone())))
        .collect();
    tracing::info!(
        "history: {}  limits: {}",
        config.history_path,
        config.limits_cache
    );
    let store = Arc::new(
        Store::open(Path::new(&config.history_path), config.body_keep, config.body_max_bytes)
            .context("open history log")?,
    );
    let pool = Arc::new(RunPool::new(client.clone(), &config));
    let state = AppState {
        config: Arc::new(config.clone()),
        client: client.clone(),
        pool: pool.clone(),
        store: store.clone(),
        catalog_ids: Arc::new(catalog_ids),
        catalog_labels: Arc::new(catalog_labels),
        account_id,
        account_label,
        started: chrono::Utc::now().timestamp_millis(),
    };
    let listener = match tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            return Err(anyhow::anyhow!("The port {} is already in use. Pick another port in Settings, or use the suggested one.", config.port));
        }
        Err(error) => {
            return Err(anyhow::Error::new(error).context(format!("bind {}:{}", config.host, config.port)));
        }
    };
    let addr = listener.local_addr()?;
    let app = server::router(state.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });
    Ok(ProxyHandle { addr, state, shutdown: Some(tx), join: Some(join) })
}
