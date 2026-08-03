//! Process startup, configuration loading, and graceful shutdown.
//!
//! Startup loads bootstrap, user, registry, and upstream credential snapshots once, then builds
//! the HTTP router and shared upstream client. Business requests do not reread configuration files.

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
/// Loads startup snapshots, assembles the HTTP service, and waits for graceful shutdown.
async fn main() -> Result<()> {
    // Initialize the logging filter.
    init_tracing()?;

    // Load bootstrap and both private credential configurations.
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

    // Build the compile-time registry and determine the credential pools required by enabled targets.
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

    // Merge downstream and upstream keys into an immutable credential snapshot before listening.
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

    // Create the shared upstream client and read-only request state.
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
    // Bind the loopback listener and start the HTTP service with graceful shutdown.
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind OpenBridge to {listen}"))?;

    info!(%listen, %registry_version, "OpenBridge listening");
    axum::serve(listener, build_router(app_state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("OpenBridge server stopped unexpectedly")
}

/// Reads the log filter from the environment and installs the process-wide tracing subscriber.
fn init_tracing() -> Result<()> {
    // Read the environment filter and default to the info level.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // Install the global tracing subscriber while preserving startup error context.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize tracing: {error}"))
}

/// Waits for Ctrl+C and provides a cancellable graceful-shutdown future to the Axum server.
async fn shutdown_signal() {
    // Wait for Ctrl+C and report handler-installation failures instead of faking normal shutdown.
    if let Err(error) = signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl+C handler");
    }
}
