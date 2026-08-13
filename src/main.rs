//! Process startup, configuration loading, and graceful shutdown.
//!
//! Startup loads bootstrap, user, registry, and upstream bindings once, then builds the HTTP router,
//! optional telemetry exporters, shared upstream client, and expiry-driven OAuth2 worker. Business
//! requests do not reread files.

use std::{collections::BTreeSet, sync::Arc};

use anyhow::{Context, Result};
use openbridge::{
    config::{BootstrapConfig, BootstrapConfigPath, HttpLoggingConfig},
    identity::UserConfigPath,
    ingress::{GatewayState, build_router},
    observability::{GatewayMetrics, HttpJsonlWriter, TelemetryRuntime, otlp_trace_layer},
    providers::build_compiled_registry_with_active_pools,
    transport::upstream::UpstreamClient,
    upstream_credentials::UpstreamCredentialConfigPath,
};
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
/// Loads startup snapshots, assembles the HTTP service, and waits for graceful shutdown.
async fn main() -> Result<()> {
    // Load and validate bootstrap before constructing any listener or telemetry egress policy.
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;

    // Build optional trace/metrics exporters, then install local logging and reviewed trace layers.
    let telemetry = TelemetryRuntime::from_bootstrap(&bootstrap)
        .context("failed to initialize OpenBridge telemetry export")?;
    init_tracing(&telemetry)?;

    // Warn once when the operator explicitly enables local HTTP content diagnostics.
    warn_if_http_logging_enabled(bootstrap.http_logging());

    // Inject the runtime meter, retain both providers, and perform one bounded shutdown flush.
    let server_result = run_service(bootstrap, telemetry.metrics()).await;
    if let Err(error) = telemetry.shutdown().await {
        tracing::warn!(%error, "OpenBridge telemetry exporter shutdown was incomplete");
    }
    server_result
}

/// Warns that opted-in local HTTP snapshots can contain owner-controlled business content.
fn warn_if_http_logging_enabled(logging: &HttpLoggingConfig) {
    // Keep the default path silent and report the exact enabled dimensions without any HTTP data.
    if logging.request_headers()
        || logging.request_body()
        || logging.response_headers()
        || logging.response_body()
    {
        tracing::warn!(
            request_headers = logging.request_headers(),
            request_body = logging.request_body(),
            response_headers = logging.response_headers(),
            response_body = logging.response_body(),
            "local authenticated HTTP content logging is enabled; use only with controlled development traffic"
        );
    }
}

/// Loads private snapshots, serves Axum, and stops the OAuth2 worker after graceful shutdown.
async fn run_service(bootstrap: BootstrapConfig, metrics: GatewayMetrics) -> Result<()> {
    // Load both private credential configurations from bootstrap-owned paths.
    let user_configuration = UserConfigPath::new(bootstrap.users_file())
        .load()
        .context("failed to load downstream users")?;
    let upstream_configuration =
        UpstreamCredentialConfigPath::new(bootstrap.upstream_credentials_file())
            .load()
            .context("failed to load upstream credentials")?;

    // Derive a redacted active-pool set without copying any credential material into the registry.
    let active_pool_ids = upstream_configuration
        .active_pool_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    // Build the compile-time registry and configuration-only availability report from the active pools.
    let registry = build_compiled_registry_with_active_pools(bootstrap, &active_pool_ids)
        .context("failed to build OpenBridge code registry")?;
    let availability_report = registry.configuration_availability(&active_pool_ids);

    // Determine the credential pools required by the remaining enabled Targets.
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
    let oauth2_credentials = upstream_configuration
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

    // Display the redacted configuration snapshot only after every private source passes validation.
    tracing::info!("\n{availability_report}");

    // Initialize the content writer before binding the listener so an unusable sink fails startup.
    let http_jsonl_writer = registry
        .http_logging()
        .http_jsonl_directory()
        .map(|directory| {
            HttpJsonlWriter::new(directory.to_path_buf())
                .map_err(anyhow::Error::msg)
                .context("failed to initialize HTTP JSONL logging")
        })
        .transpose()?;

    // Create the shared upstream client and read-only request state.
    let listen = registry.listen();
    let registry_version = registry.version().as_str().to_owned();
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    let oauth2_credentials = Arc::new(oauth2_credentials);
    let app_state = GatewayState::new_with_oauth2_credentials(
        Arc::new(registry),
        Arc::new(upstream),
        Arc::new(users),
        credentials,
        Arc::clone(&oauth2_credentials),
    )
    .with_metrics(metrics)
    .with_http_jsonl_writer(http_jsonl_writer.clone());
    // Bind the loopback listener and start the HTTP service with graceful shutdown.
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind OpenBridge to {listen}"))?;

    info!(%listen, %registry_version, "OpenBridge listening");
    let refresh_worker = (oauth2_credentials.configured_provider_count() > 0)
        .then(|| tokio::spawn(Arc::clone(&oauth2_credentials).run_refresh_scheduler()));
    let server_result = axum::serve(listener, build_router(app_state))
        .with_graceful_shutdown(shutdown_signal())
        .await;

    // Cancel the credential worker after the HTTP service reaches its terminal state.
    if let Some(refresh_worker) = refresh_worker {
        refresh_worker.abort();
        let _ = refresh_worker.await;
    }
    if let Some(writer) = http_jsonl_writer
        && let Err(error) = writer.shutdown()
    {
        tracing::warn!(%error, "OpenBridge HTTP JSONL shutdown was incomplete");
    }
    server_result.context("OpenBridge server stopped unexpectedly")
}

/// Installs local formatting plus the optional allowlisted OpenTelemetry span layer.
fn init_tracing(telemetry: &TelemetryRuntime) -> Result<()> {
    // Read the environment filter and default to the info level.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Compose the existing local formatter before optionally attaching the filtered OTLP trace layer.
    let local_logs = tracing_subscriber::fmt::layer().with_filter(filter);
    let subscriber = tracing_subscriber::registry().with(local_logs);
    match telemetry.tracer() {
        Some(tracer) => subscriber.with(otlp_trace_layer(tracer)).try_init(),
        None => subscriber.try_init(),
    }
    .map_err(|error| anyhow::anyhow!("failed to initialize tracing: {error}"))
}

/// Waits for Ctrl+C and provides a cancellable graceful-shutdown future to the Axum server.
async fn shutdown_signal() {
    // Wait for Ctrl+C and report handler-installation failures instead of faking normal shutdown.
    if let Err(error) = signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl+C handler");
    }
}
