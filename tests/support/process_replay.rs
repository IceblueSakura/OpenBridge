//! Minimal replay runner for the canonical corpus and a real loopback HTTP SUT.
//!
//! The facade owns the production Router harness and safe observations. HTTP-error and streaming
//! lifecycle scenarios remain in dedicated children. Replays use only loopback sockets, synthetic
//! credentials, and checked-in fixtures; they never call a real Provider or retain business bodies.

use std::{net::SocketAddr, sync::Arc};

use axum::{Router, body::Body};
use futures_util::future::BoxFuture;
use http::{HeaderMap, StatusCode};
use openbridge::{
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

use super::metrics::{GatewayMetricsSnapshot, ProviderMetricSnapshot, TestMetrics};

mod http_error;
mod streaming;

#[allow(unused_imports)]
pub use http_error::replay_http_error_case;
#[allow(unused_imports)]
pub use streaming::{
    replay_cancel_after_output_case, replay_eof_before_terminal_case,
    replay_transport_error_after_output_case,
};

/// Safe summary of one loopback replay.
pub struct ReplayObservation {
    /// Final HTTP status returned by OpenBridge.
    pub status: StatusCode,
    /// Safe downstream Content-Type retained by OpenBridge.
    pub content_type: Option<String>,
    /// Safe `Retry-After` value retained by OpenBridge.
    pub retry_after: Option<String>,
    /// Safe remaining-request rate-limit value retained by OpenBridge.
    pub rate_limit_remaining_requests: Option<String>,
    /// Number of attempts actually received by the mock upstream.
    pub upstream_attempts: usize,
    /// Whether each upstream JSON request matches the canonical expectation.
    pub upstream_request_matches: Vec<bool>,
    /// Whether the downstream body bytes match the canonical expectation.
    pub downstream_body_matches: bool,
    /// Whether a Native downstream stream exactly preserves the canonical upstream bytes.
    pub downstream_stream_matches_upstream: bool,
    /// Whether the downstream HTTP body ended with a transport error.
    pub downstream_transport_error: bool,
    /// Whether downstream cancellation dropped the pending upstream body.
    pub upstream_cancelled: bool,
    /// Low-cardinality gateway metric totals captured after the replay terminal.
    pub gateway_metrics: GatewayMetricsSnapshot,
    /// Low-cardinality Provider metric totals captured after the replay terminal.
    pub provider_metrics: Vec<ProviderMetricSnapshot>,
}

pub(crate) struct GatewayHarness {
    pub(crate) address: SocketAddr,
    pub(crate) task: JoinHandle<()>,
    pub(crate) metrics: TestMetrics,
}

struct LoopbackReplayTransport {
    base_url: String,
    client: reqwest::Client,
}

impl UpstreamTransport for LoopbackReplayTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Fix the loopback origin created by the test runner while preserving the adapter-generated relative path.
        let url = format!("{}{}", self.base_url, request.relative_uri());
        let method = request.method().clone();
        let body = request.body().clone();
        let client = self.client.clone();

        // Send the request through a real HTTP client and socket while preserving streaming body boundaries.
        Box::pin(async move {
            let response = client
                .request(method, url)
                .headers(headers)
                .body(body)
                .send()
                .await
                .map_err(TransportError::Request)?;
            let status = response.status();
            let headers = response.headers().clone();
            Ok(UpstreamResponse::new(
                status,
                headers,
                Body::from_stream(response.bytes_stream()),
            ))
        })
    }
}

/// Starts the production Router against one test-owned loopback upstream origin.
pub(super) async fn start_gateway(upstream_address: SocketAddr) -> GatewayHarness {
    start_gateway_with_definition(upstream_address, default_replay_definition()).await
}

/// Builds the standard replay registry with Responses streaming enabled.
fn default_replay_definition() -> openbridge::registry::RegistryConfig {
    let mut definition = super::definition("process-replay", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.streaming = true;
        capabilities.terminal_usage = true;
    }
    definition
}

/// Starts the production Router with an explicit registry definition for one replay harness.
pub(crate) async fn start_gateway_with_definition(
    upstream_address: SocketAddr,
    definition: openbridge::registry::RegistryConfig,
) -> GatewayHarness {
    // Build the fixed registry, synthetic credentials, and observed test transport.
    let registry =
        openbridge::registry::build_registry(super::bootstrap(super::BOOTSTRAP), definition)
            .expect("replay registry must be valid");
    let transport = LoopbackReplayTransport {
        base_url: format!("http://{upstream_address}"),
        client: reqwest::Client::new(),
    };
    let (users, credentials) = super::users_and_credentials(
        "downstream-token-0000000000000000",
        &registry,
        "synthetic-upstream-token",
    );
    let metrics = TestMetrics::new();
    let state = GatewayState::new(Arc::new(registry), Arc::new(transport), users, credentials)
        .with_metrics(metrics.instruments());

    // Bind and spawn the downstream listener before returning its explicit lifecycle handles.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("OpenBridge replay listener must bind loopback");
    let address = listener
        .local_addr()
        .expect("OpenBridge replay address must exist");
    let task = spawn_server(listener, build_router(state));
    GatewayHarness {
        address,
        task,
        metrics,
    }
}

pub(crate) fn read_json(path: std::path::PathBuf) -> Value {
    // Read and parse the canonical JSON artifact, binding errors directly to the test fixture.
    let bytes = std::fs::read(path).expect("canonical JSON artifact must be readable");
    serde_json::from_slice(&bytes).expect("canonical JSON artifact must be valid")
}

pub(crate) fn spawn_server(listener: TcpListener, router: Router) -> JoinHandle<()> {
    // Run the loopback server in an independent task and explicitly abort it on test completion.
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("loopback replay server must remain valid");
    })
}
