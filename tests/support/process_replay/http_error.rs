//! Replays canonical Chat and Responses HTTP errors through the production Router.

use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{CONTENT_TYPE, HeaderName},
};
use serde_json::{Map, Value};
use tokio::net::TcpListener;

use super::{GatewayHarness, ReplayObservation, read_json, spawn_server, start_gateway};

#[derive(Clone)]
struct MockUpstreamState {
    expected_request: Value,
    response_status: StatusCode,
    response_headers: HeaderMap,
    response_body: Bytes,
    observations: Arc<Mutex<Vec<bool>>>,
}

#[derive(Clone, Copy)]
enum NativeEndpoint {
    Chat,
    Responses,
}

impl NativeEndpoint {
    /// Resolves one fixed downstream/upstream path from a validated canonical case prefix.
    fn from_case_id(case_id: &str) -> Self {
        if case_id.starts_with("chat_native.") {
            Self::Chat
        } else if case_id.starts_with("responses_native.") {
            Self::Responses
        } else {
            panic!("HTTP replay case id must be a canonical Chat or Responses identifier");
        }
    }

    /// Returns the only trusted relative endpoint represented by this protocol family.
    fn path(self) -> &'static str {
        match self {
            Self::Chat => "/v1/chat/completions",
            Self::Responses => "/v1/responses",
        }
    }

    /// Returns the canonical direction metadata associated with this endpoint.
    fn direction(self) -> &'static str {
        match self {
            Self::Chat => "chat_native",
            Self::Responses => "responses_native",
        }
    }
}

struct CanonicalHttpErrorCase {
    endpoint: NativeEndpoint,
    client_request: Value,
    expected_upstream_request: Value,
    upstream_status: StatusCode,
    upstream_headers: HeaderMap,
    upstream_body: Bytes,
    expected_client_response: Bytes,
}

/// Replays a canonical Chat or Responses HTTP error and returns a body-free safe summary.
pub async fn replay_http_error_case(case_id: &str) -> ReplayObservation {
    // Load fixed wire artifacts and validated HTTP metadata for one canonical case.
    let CanonicalHttpErrorCase {
        endpoint,
        client_request,
        expected_upstream_request,
        upstream_status,
        upstream_headers,
        upstream_body,
        expected_client_response,
    } = load_http_error_case(case_id);

    // Start a mock upstream listener on only the case's trusted protocol endpoint.
    let observations = Arc::new(Mutex::new(Vec::new()));
    let upstream_state = MockUpstreamState {
        expected_request: expected_upstream_request,
        response_status: upstream_status,
        response_headers: upstream_headers,
        response_body: upstream_body,
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
            .route(endpoint.path(), post(mock_http_error))
            .with_state(upstream_state),
    );

    // Start the production Router with the mock origin and in-memory metric exporter.
    let GatewayHarness {
        address: gateway_address,
        task: gateway_task,
        metrics,
    } = start_gateway(upstream_address).await;

    // Send the canonical request through a real downstream HTTP client.
    let response = reqwest::Client::new()
        .post(format!("http://{gateway_address}{}", endpoint.path()))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(serde_json::to_vec(&client_request).expect("canonical request must encode"))
        .send()
        .await
        .expect("OpenBridge replay request must complete");

    // Capture only allowlisted response metadata and compare body bytes without retaining them.
    let status = response.status();
    let content_type = header_string(response.headers(), CONTENT_TYPE);
    let retry_after = header_string(response.headers(), HeaderName::from_static("retry-after"));
    let rate_limit_remaining_requests = header_string(
        response.headers(),
        HeaderName::from_static("x-ratelimit-remaining-requests"),
    );
    let body = response
        .bytes()
        .await
        .expect("OpenBridge replay response body must be readable");
    let downstream_body_matches = body == expected_client_response;

    // Stop both listeners and return only comparisons, counts, and low-cardinality metrics.
    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let gateway_metrics = metrics.snapshot();
    let provider_metrics = metrics.provider_snapshots();
    ReplayObservation {
        status,
        content_type,
        retry_after,
        rate_limit_remaining_requests,
        upstream_attempts: upstream_request_matches.len(),
        upstream_request_matches,
        downstream_body_matches,
        downstream_stream_matches_upstream: false,
        downstream_transport_error: false,
        upstream_cancelled: false,
        gateway_metrics,
        provider_metrics,
    }
}

async fn mock_http_error(
    State(state): State<MockUpstreamState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Compare request JSON while retaining only booleans and never echoing bodies or authentication headers.
    let matches =
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| value == state.expected_request);
    state
        .observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(matches);

    // Return the same canonical status, safe headers, and raw error bytes for every attempt.
    let mut response = Response::builder()
        .status(state.response_status)
        .body(Body::from(state.response_body))
        .expect("canonical mock response must build");
    response.headers_mut().extend(state.response_headers);
    response
}

fn load_http_error_case(case_id: &str) -> CanonicalHttpErrorCase {
    // Restrict fixture lookup to canonical Native identifiers under the fixed case root.
    let endpoint = NativeEndpoint::from_case_id(case_id);
    assert!(
        case_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        }),
        "HTTP replay case id contains an invalid path character"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/native")
        .join(case_id);
    let case = read_json(root.join("case.json"));
    assert_eq!(case["id"].as_str(), Some(case_id));
    assert_eq!(case["direction"].as_str(), Some(endpoint.direction()));

    // Validate the canonical before-output HTTP-error envelope before constructing a response.
    let transport = case["transport"]
        .as_object()
        .expect("canonical HTTP error transport metadata must be an object");
    assert_eq!(transport["upstream_end"].as_str(), Some("error_response"));
    assert_eq!(transport["client_end"].as_str(), Some("error_response"));
    assert_eq!(
        transport["failure_phase"].as_str(),
        Some("before_first_output")
    );
    let upstream_status = status_code(transport, "upstream_http_status");
    assert!(
        !upstream_status.is_success(),
        "HTTP error replay requires a non-success status"
    );
    assert_eq!(
        status_code(transport, "client_http_status"),
        upstream_status,
        "canonical Native HTTP error must preserve status"
    );

    // Build only the upstream headers explicitly declared by the transport oracle.
    let content_type = transport["upstream_content_type"]
        .as_str()
        .expect("canonical upstream Content-Type must exist");
    assert_eq!(
        transport["client_content_type"].as_str(),
        Some(content_type),
        "canonical Native HTTP error must preserve Content-Type"
    );
    let mut upstream_headers = HeaderMap::new();
    upstream_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_bytes(content_type.as_bytes())
            .expect("canonical upstream Content-Type must be valid"),
    );
    append_declared_headers(transport, &mut upstream_headers);

    // Resolve only declared single-file artifacts and load response bodies as opaque bytes.
    let artifacts = case["artifacts"]
        .as_object()
        .expect("canonical HTTP error artifacts must be an object");
    CanonicalHttpErrorCase {
        endpoint,
        client_request: read_json(artifact_path(&root, artifacts, "client_request")),
        expected_upstream_request: read_json(artifact_path(
            &root,
            artifacts,
            "expected_upstream_request",
        )),
        upstream_status,
        upstream_headers,
        upstream_body: read_artifact_bytes(&root, artifacts, "upstream_response"),
        expected_client_response: read_artifact_bytes(&root, artifacts, "expected_client_response"),
    }
}

/// Reads one required HTTP status from canonical transport metadata.
fn status_code(transport: &Map<String, Value>, field: &str) -> StatusCode {
    transport[field]
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .and_then(|value| StatusCode::from_u16(value).ok())
        .unwrap_or_else(|| panic!("canonical {field} must be a valid HTTP status"))
}

/// Appends transport-oracle headers after validating every name and value.
fn append_declared_headers(transport: &Map<String, Value>, headers: &mut HeaderMap) {
    let Some(pairs) = transport.get("upstream_headers") else {
        return;
    };

    // Validate and append each declared name/value pair without accepting an object-shaped header map.
    for pair in pairs
        .as_array()
        .expect("canonical upstream headers must be pairs")
    {
        let pair = pair
            .as_array()
            .filter(|pair| pair.len() == 2)
            .expect("canonical upstream header must contain a name and value");
        let name = pair[0]
            .as_str()
            .and_then(|value| HeaderName::from_bytes(value.as_bytes()).ok())
            .expect("canonical upstream header name must be valid");
        let value = pair[1]
            .as_str()
            .and_then(|value| HeaderValue::from_bytes(value.as_bytes()).ok())
            .expect("canonical upstream header value must be valid");
        headers.append(name, value);
    }
}

/// Resolves one declared artifact without allowing nested or parent-relative paths.
fn artifact_path(root: &Path, artifacts: &Map<String, Value>, key: &str) -> PathBuf {
    let file_name = artifacts[key]
        .as_str()
        .unwrap_or_else(|| panic!("canonical artifact {key} must name one file"));
    let path = Path::new(file_name);
    let mut components = path.components();
    assert!(
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
        "canonical artifact {key} must stay inside its case directory"
    );
    assert!(
        file_name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        }),
        "canonical artifact {key} contains an invalid path character"
    );
    root.join(path)
}

/// Reads one declared response artifact as opaque bytes.
fn read_artifact_bytes(root: &Path, artifacts: &Map<String, Value>, key: &str) -> Bytes {
    std::fs::read(artifact_path(root, artifacts, key))
        .map(Bytes::from)
        .unwrap_or_else(|error| panic!("canonical artifact {key} must be readable: {error}"))
}

/// Copies one UTF-8 response header into the body-free observation.
fn header_string(headers: &HeaderMap, name: HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}
