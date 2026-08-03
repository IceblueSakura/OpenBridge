//! Minimal replay runner for the canonical corpus and a real loopback HTTP SUT.
//!
//! The runner creates an explicit loopback address only in the test process while the production
//! registry remains HTTPS-only. It does not read `.env`, call a real Provider, or write
//! authentication headers or business bodies to observations.

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use futures_util::future::BoxFuture;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

/// Safe summary of one loopback replay.
pub struct ReplayObservation {
    /// Final HTTP status returned by OpenBridge.
    pub status: StatusCode,
    /// Safe `Retry-After` value retained by OpenBridge.
    pub retry_after: Option<String>,
    /// Number of attempts actually received by the mock upstream.
    pub upstream_attempts: usize,
    /// Whether each upstream JSON request matches the canonical expectation.
    pub upstream_request_matches: Vec<bool>,
    /// Whether the downstream JSON body matches the canonical expectation.
    pub downstream_body_matches: bool,
}

#[derive(Clone)]
struct MockUpstreamState {
    expected_request: Value,
    response_body: Bytes,
    observations: Arc<Mutex<Vec<bool>>>,
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

/// Replays a canonical Responses 429 case and returns a summary without bodies or credentials.
pub async fn replay_rate_limit_case(case_id: &str) -> ReplayObservation {
    // Load four canonical wire artifacts from the fixed corpus directory.
    assert_eq!(case_id, "responses_native.rate_limit.non_stream");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/native")
        .join(case_id);
    let client_request = read_json(root.join("client-request.json"));
    let expected_upstream = read_json(root.join("expected-upstream-request.json"));
    let upstream_body = std::fs::read(root.join("upstream-response.json"))
        .expect("canonical upstream response must be readable");
    let expected_client = read_json(root.join("expected-client-response.json"));

    // Start the mock upstream listener that records actual HTTP requests.
    let observations = Arc::new(Mutex::new(Vec::new()));
    let upstream_state = MockUpstreamState {
        expected_request: expected_upstream,
        response_body: Bytes::from(upstream_body),
        observations: observations.clone(),
    };
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock upstream must bind loopback");
    let upstream_address = upstream_listener
        .local_addr()
        .expect("mock upstream address must exist");
    let upstream_task = spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/responses", post(mock_rate_limit))
            .with_state(upstream_state),
    );

    // Start the SUT with the production Router and adapter, replacing the origin only inside test transport.
    let registry = openbridge::registry::build_registry(
        super::bootstrap(super::BOOTSTRAP),
        super::definition("process-replay", "public-model", "upstream-model"),
    )
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
    let state = GatewayState::new(Arc::new(registry), Arc::new(transport), users, credentials);
    let gateway_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("OpenBridge replay listener must bind loopback");
    let gateway_address = gateway_listener
        .local_addr()
        .expect("OpenBridge replay address must exist");
    let gateway_task = spawn_server(gateway_listener, build_router(state));

    // Send the canonical request through a real downstream HTTP client and read the final safe response.
    let response = reqwest::Client::new()
        .post(format!("http://{gateway_address}/v1/responses"))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(serde_json::to_vec(&client_request).expect("canonical request must encode"))
        .send()
        .await
        .expect("OpenBridge replay request must complete");
    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .bytes()
        .await
        .expect("OpenBridge replay response body must be readable");
    let body: Value =
        serde_json::from_slice(&body).expect("OpenBridge replay response must be JSON");

    // Stop both listeners and return only a comparison summary without sensitive content.
    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    ReplayObservation {
        status,
        retry_after,
        upstream_attempts: upstream_request_matches.len(),
        upstream_request_matches,
        downstream_body_matches: body == expected_client,
    }
}

async fn mock_rate_limit(
    State(state): State<MockUpstreamState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Compare request JSON while observations retain only booleans and never echo bodies or authentication headers.
    let matches =
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| value == state.expected_request);
    state
        .observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(matches);

    // Return the same canonical 429 and Retry-After for every attempt.
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header(CONTENT_TYPE, "application/json")
        .header("retry-after", "1")
        .body(Body::from(state.response_body))
        .expect("canonical mock response must build")
}

fn read_json(path: std::path::PathBuf) -> Value {
    // Read and parse the canonical JSON artifact, binding errors directly to the test fixture.
    let bytes = std::fs::read(path).expect("canonical JSON artifact must be readable");
    serde_json::from_slice(&bytes).expect("canonical JSON artifact must be valid")
}

fn spawn_server(listener: TcpListener, router: Router) -> JoinHandle<()> {
    // Run the loopback server in an independent task and explicitly abort it on test completion.
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("loopback replay server must remain valid");
    })
}
