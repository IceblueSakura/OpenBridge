//! 进程启动、配置装载与优雅关闭。
//!
//! 启动阶段一次性加载 bootstrap、用户、代码注册表与上下游 credential 快照，再构造
//! HTTP router 和共享 upstream client；业务请求不重新读取配置文件。

use std::sync::Arc;

use anyhow::{Context, Result};
use openbridge::{
    config::BootstrapConfigPath,
    identity::UserConfigPath,
    ingress::{GatewayState, build_router},
    providers::build_compiled_registry,
    transport::upstream::UpstreamClient,
    upstream_credentials::UpstreamCredentialConfigPath,
};
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志过滤器。
    init_tracing()?;

    // 读取 bootstrap 与两份私有 credential 配置。
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let user_configuration = UserConfigPath::new(bootstrap.users_file())
        .load()
        .context("failed to load downstream users")?;
    let upstream_configuration =
        UpstreamCredentialConfigPath::new(bootstrap.upstream_credentials_file())
            .load()
            .context("failed to load upstream credentials")?;

    // 构建编译期 registry，并确定启用 target 实际要求的 credential pool。
    let registry =
        build_compiled_registry(bootstrap).context("failed to build OpenBridge code registry")?;
    let required_pool_ids = registry
        .credential_pool_ids()
        .filter(|pool_id| {
            registry.upstream_target_ids().any(|target_id| {
                let target = registry
                    .upstream_target(target_id)
                    .expect("registry target id must resolve");
                target.enabled() && target.credential_pool_id() == *pool_id
            })
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();

    // 合并上下游 Key，监听前完成不可变 credential 快照。
    let (users, mut credential_builder) = user_configuration.into_parts();
    upstream_configuration
        .load_into_for(
            &mut credential_builder,
            &registry,
            required_pool_ids.iter().map(String::as_str),
        )
        .context("failed to bind upstream credentials to the code registry")?;
    let credentials = credential_builder.build();
    credentials
        .validate_registry(&registry)
        .context("upstream credential pools violate registry state-affinity constraints")?;
    let credentials = Arc::new(credentials);

    // 创建共享上游 client 与只读请求状态。
    let listen = registry.listen();
    let registry_version = registry.version().as_str().to_owned();
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    let app_state = GatewayState::new(
        Arc::new(registry),
        Arc::new(upstream),
        Arc::new(users),
        credentials,
    );
    // 绑定 loopback listener 并启动带优雅关闭的 HTTP 服务。
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
    // 读取环境中的日志过滤器，缺省使用 info 级别。
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // 安装全局 tracing subscriber，失败时保留启动错误上下文。
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize tracing: {error}"))
}

async fn shutdown_signal() {
    // 等待 Ctrl+C，并将信号安装失败记录为错误而不伪造正常关闭。
    if let Err(error) = signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl+C handler");
    }
}
