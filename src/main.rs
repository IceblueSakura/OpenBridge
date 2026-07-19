use std::{env, fs, sync::Arc};

use anyhow::{Context, Result};
use openbridge::{
    config::{ConfigManager, load_registry},
    ingress::build_router,
};
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_BOOTSTRAP_PATH: &str = "config/bootstrap.toml";
const DEFAULT_ROUTES_PATH: &str = "config/routes.toml";

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;

    let bootstrap_path =
        env::var("OPENBRIDGE_BOOTSTRAP_CONFIG").unwrap_or_else(|_| DEFAULT_BOOTSTRAP_PATH.into());
    let routes_path =
        env::var("OPENBRIDGE_ROUTES_CONFIG").unwrap_or_else(|_| DEFAULT_ROUTES_PATH.into());
    let bootstrap = fs::read_to_string(&bootstrap_path)
        .with_context(|| format!("failed to read bootstrap config '{bootstrap_path}'"))?;
    let routes = fs::read_to_string(&routes_path)
        .with_context(|| format!("failed to read route config '{routes_path}'"))?;
    let snapshot = load_registry(&bootstrap, &routes).context("configuration validation failed")?;
    let listen = snapshot.listen();
    let config_version = snapshot.version().as_str().to_owned();
    let config = Arc::new(ConfigManager::new(snapshot));
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind OpenBridge to {listen}"))?;

    info!(%listen, %config_version, "OpenBridge listening");
    axum::serve(listener, build_router(config))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("OpenBridge server stopped unexpectedly")
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize tracing: {error}"))
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl+C handler");
    }
}
