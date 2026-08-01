//! 进程启动、配置装载与优雅关闭。
//!
//! 启动阶段一次性加载 bootstrap、构建代码注册表、HTTP router 和共享 upstream client；
//! credential 只保留环境变量名称，实际 API key 在每个业务请求发送前才解析。

use std::{env, sync::Arc};

use anyhow::{Context, Result};
use openbridge::{
    config::{BootstrapConfigPath, load_optional_dotenv},
    ingress::{DownstreamCredential, GatewayState, build_router},
    providers::build_compiled_registry,
    transport::upstream::UpstreamClient,
};
use secrecy::SecretString;
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    load_optional_dotenv().context("failed to load optional .env file")?;
    init_tracing()?;

    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let registry =
        build_compiled_registry(bootstrap).context("failed to build OpenBridge code registry")?;
    let listen = registry.listen();
    let registry_version = registry.version().as_str().to_owned();
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    let downstream_token = env::var("OPENBRIDGE_DOWNSTREAM_TOKEN")
        .context("OPENBRIDGE_DOWNSTREAM_TOKEN must be configured")?;
    if downstream_token.is_empty() {
        anyhow::bail!("OPENBRIDGE_DOWNSTREAM_TOKEN must not be empty");
    }
    let app_state = GatewayState::with_environment_credentials(
        Arc::new(registry),
        upstream,
        DownstreamCredential::new(SecretString::from(downstream_token)),
    );
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind OpenBridge to {listen}"))?;

    info!(%listen, %registry_version, "OpenBridge listening");
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
