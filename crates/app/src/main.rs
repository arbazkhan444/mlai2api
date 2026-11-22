mod gui;

use clap::{Parser, Subcommand};
use mlai_bridge_core::auth;
use mlai_bridge_core::config::{default_config_path, new_api_key, Config};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mlai-bridge", version, about = "OpenAI-compatible local bridge to ML AI Code")]
struct Cli {
    /// config.json to use (default: ~/.mlai-bridge/config.json, then ./config.json)
    #[arg(long, global = true)]
    config: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Open the desktop control panel (default)
    Gui,
    /// Run the proxy headless in this terminal
    Serve {
        #[arg(long)]
        port: Option<u16>,
    },
    /// Write a config.json with a bridge key and discovered project
    Init {
        #[arg(long)]
        force: bool,
    },
    /// Check sign-in, project and config, then exit
    Doctor,
    /// Drive the GUI's own command path without opening a window
    SelfTest,
}

fn main() {
    let cli = Cli::parse();
    let config_path = cli.config.map(PathBuf::from).unwrap_or_else(default_config_path);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let result = match cli.command.unwrap_or(Command::Gui) {
        Command::Gui => gui::run(config_path),
        Command::Serve { port } => serve(config_path, port),
        Command::Init { force } => init(config_path, force),
        Command::Doctor => doctor(config_path),
        Command::SelfTest => gui::self_test(config_path),
    };
    if let Err(error) = result {
        eprintln!("error: {:#}", error);
        std::process::exit(1);
    }
}

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread().enable_all().build()?)
}

fn serve(config_path: PathBuf, port: Option<u16>) -> anyhow::Result<()> {
    let (mut config, exists) = Config::load(&config_path)?;
    if !exists {
        anyhow::bail!("No config at {}. Run mlai-bridge init first.", config_path.display());
    }
    if let Some(port) = port {
        config.port = port;
    }
    let fallback = auth::load(&config.server).unwrap_or(auth::Credential {
        token: String::new(),
        server: config.server.clone(),
        store: "none".into(),
        label: None,
        expires: None,
    });
    let accounts = mlai_bridge_core::accounts::Accounts::open(std::path::Path::new(&config.accounts_path))?;
    let (credential, account_id, account_label) = accounts.active_credential(&fallback)?;
    if credential.token.is_empty() {
        anyhow::bail!("No active account. Add one in the app, or run mlai login.");
    }
    println!("mlai-bridge: account {} ({})", account_label, credential.server);
    let rt = runtime()?;
    rt.block_on(async move {
        let handle = mlai_bridge_core::start_proxy(config, credential, account_id, account_label).await?;
        println!("listening on {}/v1", handle.base_url());
        println!("press Ctrl-C to stop");
        let _ = tokio::signal::ctrl_c().await;
        handle.stop().await;
        Ok::<(), anyhow::Error>(())
    })
}

fn init(config_path: PathBuf, force: bool) -> anyhow::Result<()> {
    let (mut config, exists) = Config::load(&config_path)?;
    if exists && !force {
        anyhow::bail!("{} already exists (use --force to overwrite)", config_path.display());
    }
    config.api_key = new_api_key();
    if let Some((project, folder)) = discover_project() {
        config.project = project;
        config.folder = folder;
    }
    config.save(&config_path)?;
    println!("wrote {}", config_path.display());
    println!("bridge key: {}", config.api_key);
    if config.project.is_empty() {
        println!("warning: no laptop project id found - open a chat once with mlai, then run init again");
    }
    Ok(())
}

fn discover_project() -> Option<(String, String)> {
    let home = auth::mlai_home();
    let sessions = home.join("sessions");
    let mut best: Option<(std::time::SystemTime, String, String)> = None;
    if let Ok(entries) = std::fs::read_dir(&sessions) {
        for entry in entries.flatten() {
            let meta_path = entry.path().join("session.json");
            let Ok(text) = std::fs::read_to_string(&meta_path) else { continue };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let project = value.get("project").and_then(|v| v.as_str()).unwrap_or_default();
            let folder = value.get("folder").and_then(|v| v.as_str()).unwrap_or_default();
            if project.is_empty() {
                continue;
            }
            let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().map(|(t, _, _)| modified > *t).unwrap_or(true) {
                best = Some((modified, project.to_string(), folder.to_string()));
            }
        }
    }
    if let Some((_, project, folder)) = best {
        return Some((project, folder));
    }
    let projects = std::fs::read_to_string(home.join("projects.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&projects).ok()?;
    let (root, entry) = value.as_object()?.iter().next()?;
    let id = entry.get("id")?.as_str()?.to_string();
    let folder = PathBuf::from(root)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    Some((id, folder))
}

fn doctor(config_path: PathBuf) -> anyhow::Result<()> {
    let (config, exists) = Config::load(&config_path)?;
    println!("config: {} ({})", config_path.display(), if exists { "found" } else { "missing" });
    println!("server: {}", config.server);
    println!("project: {}  folder: {}", config.project, config.folder);
    let credential = auth::load(&config.server)?;
    println!("credential: {} ({} chars)", credential.store, credential.token.len());
    let rt = runtime()?;
    rt.block_on(async {
        let client = mlai_bridge_core::mlai::MlaiClient::new(&config.server, &credential.token);
        match client.whoami().await {
            Ok(who) => println!(
                "account: {} ({})",
                who.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                who.get("label").and_then(|v| v.as_str()).unwrap_or("?")
            ),
            Err(error) => println!("account check failed: {:#}", error),
        }
        match client.catalog().await {
            Ok(catalog) => println!("catalog: {} models", catalog.models.len()),
            Err(error) => println!("catalog check failed: {:#}", error),
        }
    });
    Ok(())
}
