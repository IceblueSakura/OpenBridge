//! 进程启动、配置装载与优雅关闭。
//!
//! 启动阶段一次性构造 bootstrap-bound HTTP router 和共享 upstream client；路由中的
//! credential 只保留 `env://` 引用，实际 API key 在每个业务请求发送前才解析。

use std::{env, sync::Arc};

use anyhow::{Context, Result};
use openbridge::{
    config::{ConfigManager, ConfigPaths},
    ingress::{AppState, StaticBearerCredential, build_router},
    transport::upstream::UpstreamClient,
};
use secrecy::SecretString;
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;

    let snapshot = ConfigPaths::from_environment()
        .load()
        .context("failed to load OpenBridge configuration")?;
    let listen = snapshot.listen();
    let config_version = snapshot.version().as_str().to_owned();
    let upstream = UpstreamClient::new(
        snapshot.upstream_policy().connect_timeout(),
        snapshot.upstream_policy().pool_idle_timeout(),
        snapshot.upstream_policy().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    let downstream_token = env::var("OPENBRIDGE_DOWNSTREAM_TOKEN")
        .context("OPENBRIDGE_DOWNSTREAM_TOKEN must be configured")?;
    if downstream_token.is_empty() {
        anyhow::bail!("OPENBRIDGE_DOWNSTREAM_TOKEN must not be empty");
    }
    let config = Arc::new(ConfigManager::new(snapshot));
    let app_state = AppState::with_environment_credentials(
        config,
        upstream,
        StaticBearerCredential::new(SecretString::from(downstream_token)),
    );
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind OpenBridge to {listen}"))?;

    info!(%listen, %config_version, "OpenBridge listening");
    axum::serve(listener, build_router(app_state))
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
