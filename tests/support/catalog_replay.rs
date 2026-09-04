//! Catalog-driven replay of canonical corpus cases through the production Router.
//!
//! Every replayable wire case is sent to the real ingress Router with a loopback mock upstream
//! that serves the case's canonical artifacts. The dedicated cancellation and post-output
//! transport-error lifecycles keep their specialized harnesses; this module asserts that the
//! remaining catalog is fully covered and matches production behavior.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use futures_util::StreamExt;
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;
use tokio::sync::Mutex;

use super::process_replay::{
    GatewayHarness, read_json, spawn_server, start_gateway_with_definition,
};

/// Canonical testdata corpus root.
pub fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

/// One replayable corpus case with its loaded artifacts.
pub struct ReplayCase {
    pub id: String,
    pub direction: String,
    pub classification: String,
    pub outcome: String,
    pub stream: bool,
    pub expected_attempts: u64,
    pub dir: PathBuf,
    pub case: Value,
    pub client_request: Bytes,
    pub expected_upstream_request: Option<Value>,
    pub upstream_body: Option<Bytes>,
    pub upstream_is_sse: bool,
    pub expected_client: Option<Bytes>,
    pub expected_client_is_json: bool,
}

/// Observable result of one catalog replay.
pub struct ReplayProbe {
    pub status: StatusCode,
    pub content_type: Option<String>,
    pub attempts: usize,
    pub upstream_request_matches: Vec<bool>,
    pub body: Vec<u8>,
    pub body_error: bool,
    pub body_error_message: String,
}

impl ReplayCase {
    /// Whether the replay uses the dedicated lifecycle harnesses instead of this generic path.
    pub fn is_lifecycle_delegated(&self) -> bool {
        matches!(
            self.id.as_str(),
            "responses_native.cancel.after_output"
                | "responses_native.transport_error.after_output"
        )
    }

    /// Whether this case still encodes a proposed oracle that diverges from production behavior.
    ///
    /// These cases expect a synthesized terminal event after a stream violation, while the
    /// production Router terminates the stream without injecting synthetic events. The replay
    /// locks the production fail-closed behavior instead of the proposed oracle bytes until the
    /// corpus artifacts receive an explicit product decision.
    pub fn is_known_divergence(&self) -> bool {
        matches!(
            self.id.as_str(),
            "responses_native.event_type_conflict"
                | "responses_native.terminal_violation"
                | "responses_to_chat.incomplete_arguments.stream"
        )
    }

    /// Whether this case's direction crosses the Event IR Bridge.
    ///
    /// Bridge upstream-request artifacts encode converter-layer output and are locked by
    /// `bridge_conversion_contract`; production adds normalization (instructions envelope,
    /// `store`) on top, so the catalog replay does not re-assert them at the egress layer.
    pub fn is_bridge_direction(&self) -> bool {
        matches!(
            self.direction.as_str(),
            "chat_to_responses" | "responses_to_chat"
        )
    }
}

/// Discovers every catalog case directory and loads its replay artifacts.
pub fn discover_cases() -> Vec<ReplayCase> {
    let catalog = read_json(corpus_root().join("catalog.json"));
    let ids = catalog["case_ids"]
        .as_array()
        .expect("catalog case_ids must exist");
    let mut cases = Vec::new();
    for id in ids {
        let id = id.as_str().expect("catalog case id must be a string");
        cases.push(load_case(id));
    }
    cases.sort_by(|left, right| left.id.cmp(&right.id));
    cases
}

fn load_case(id: &str) -> ReplayCase {
    // Locate the case directory by recursive search within the fixed category roots.
    let mut dir = None;
    let mut pending = vec![corpus_root().join("cases")];
    while let Some(current) = pending.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&current)
            .unwrap_or_else(|_| panic!("corpus directory {} must exist", current.display()))
            .map(|entry| entry.expect("corpus entry must be readable"))
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if entry.file_name() == id && entry.path().join("case.json").exists() {
                dir = Some(entry.path());
                break;
            }
            if entry.path().is_dir() {
                pending.push(entry.path());
            }
        }
        if dir.is_some() {
            break;
        }
    }
    let dir = dir.unwrap_or_else(|| panic!("case {id} must exist under testdata/cases"));
    let case = read_json(dir.join("case.json"));
    assert_eq!(case["id"].as_str(), Some(id));

    // Load declared artifacts without assuming upstream presence for reject cases.
    let artifacts = &case["artifacts"];
    let client_request = Bytes::from(
        std::fs::read(dir.join(artifacts["client_request"].as_str().unwrap()))
            .expect("client request artifact must exist"),
    );
    let expected_upstream_request = artifacts
        .get("expected_upstream_request")
        .map(|name| read_json(dir.join(name.as_str().unwrap())));
    let upstream_artifact = artifacts
        .get("upstream_response")
        .or_else(|| artifacts.get("upstream_stream"))
        .map(|name| dir.join(name.as_str().unwrap()));
    let upstream_body = upstream_artifact
        .as_ref()
        .map(|path| Bytes::from(std::fs::read(path).expect("upstream artifact must exist")));
    let upstream_is_sse = artifacts.get("upstream_stream").is_some();
    let expected_artifact = artifacts
        .get("expected_client_response")
        .or_else(|| artifacts.get("expected_client_stream"));
    let expected_client = expected_artifact.as_ref().map(|name| {
        Bytes::from(
            std::fs::read(dir.join(name.as_str().unwrap()))
                .expect("expected client artifact must exist"),
        )
    });
    let expected_client_is_json = artifacts.get("expected_client_response").is_some();

    ReplayCase {
        id: id.to_owned(),
        direction: case["direction"].as_str().unwrap().to_owned(),
        classification: case["expectation"]["classification"]
            .as_str()
            .unwrap()
            .to_owned(),
        outcome: case["expectation"]["outcome"].as_str().unwrap().to_owned(),
        stream: case["stream"].as_bool().unwrap(),
        expected_attempts: case["expectation"]["upstream_attempts"].as_u64().unwrap(),
        dir,
        case,
        client_request,
        expected_upstream_request,
        upstream_body,
        upstream_is_sse,
        expected_client,
        expected_client_is_json,
    }
}

/// Downstream endpoint for the case's public protocol.
pub fn downstream_endpoint(case: &ReplayCase) -> &'static str {
    if case.direction.starts_with("chat") {
        "/v1/chat/completions"
    } else {
        "/v1/responses"
    }
}

#[derive(Clone)]
struct MockState {
    expected_request: Option<Value>,
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    observations: Arc<Mutex<Vec<bool>>>,
}

async fn mock_respond(
    State(state): State<MockState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Record only whether the request JSON matches; never retain request bodies.
    let matches = state.expected_request.as_ref().is_none_or(|expected| {
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| &value == expected)
    });
    state.observations.lock().await.push(matches);

    let mut response = Response::builder()
        .status(state.status)
        .body(Body::from(state.body.clone()))
        .expect("mock response must build");
    response.headers_mut().extend(state.headers.clone());
    response
}

/// Builds the enriched replay registry definition shared by every catalog replay.
pub fn replay_definition() -> openbridge::registry::RegistryConfig {
    use openbridge::core::{FunctionToolCapabilities, ToolChoiceMode};
    use openbridge::registry::UpstreamApiCapabilities;

    let mut definition = super::definition("catalog-replay", "public-model", "upstream-model");
    for api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.streaming = true;
                capabilities.stream_usage = true;
                capabilities.function_tools = Some(FunctionToolCapabilities {
                    choice_modes: &[
                        ToolChoiceMode::None,
                        ToolChoiceMode::Auto,
                        ToolChoiceMode::Required,
                        ToolChoiceMode::Named,
                    ],
                    parallel_calls: true,
                    strict_schema: true,
                });
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.streaming = true;
                capabilities.terminal_usage = true;
                capabilities.function_tools = Some(FunctionToolCapabilities {
                    choice_modes: &[
                        ToolChoiceMode::None,
                        ToolChoiceMode::Auto,
                        ToolChoiceMode::Required,
                        ToolChoiceMode::Named,
                    ],
                    parallel_calls: true,
                    strict_schema: true,
                });
            }
            UpstreamApiCapabilities::Embeddings(_)
            | UpstreamApiCapabilities::ImagesGenerations(_) => {}
        }
    }
    // Advertise the ordinary parameters exercised by the corpus so preflight reaches transport.
    super::generation_profile_mut(&mut definition.models[0]).supported_parameters = vec![
        "max_output_tokens".to_owned(),
        "tools".to_owned(),
        "tool_choice".to_owned(),
        "parallel_tool_calls".to_owned(),
    ];
    definition
}

/// Builds the bridge-direction variant of the replay definition for one case.
pub fn replay_definition_for(case: &ReplayCase) -> openbridge::registry::RegistryConfig {
    use openbridge::core::OperationKind;

    let mut definition = replay_definition();
    match case.direction.as_str() {
        "chat_to_responses" => {
            let route = definition.public_models[0]
                .routes
                .iter_mut()
                .find(|route| route.downstream_operation == OperationKind::ChatCompletions)
                .expect("synthetic Chat route must exist");
            route.upstream_operation = OperationKind::Responses;
        }
        "responses_to_chat" => {
            let route = definition.public_models[0]
                .routes
                .iter_mut()
                .find(|route| route.downstream_operation == OperationKind::Responses)
                .expect("synthetic Responses route must exist");
            route.upstream_operation = OperationKind::ChatCompletions;
        }
        _ => {}
    }
    definition
}

/// Replays one catalog case through the production Router and returns safe observations.
pub async fn replay(case: &ReplayCase) -> ReplayProbe {
    // Reject cases must never reach upstream transport; count any call as a violation.
    let is_reject = case.upstream_body.is_none();
    let observations = Arc::new(Mutex::new(Vec::new()));

    // Build the loopback mock upstream from canonical transport metadata when present.
    let (status, headers, body) = if let Some(upstream_body) = &case.upstream_body {
        let transport = case.case.get("transport").cloned().unwrap_or(Value::Null);
        let status = transport["upstream_http_status"]
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .and_then(|value| StatusCode::from_u16(value).ok())
            .unwrap_or(StatusCode::OK);
        let content_type =
            transport["upstream_content_type"]
                .as_str()
                .unwrap_or(if case.upstream_is_sse {
                    "text/event-stream"
                } else {
                    "application/json"
                });
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_bytes(content_type.as_bytes())
                .expect("canonical Content-Type must be valid"),
        );
        if let Some(pairs) = transport.get("upstream_headers").and_then(Value::as_array) {
            for pair in pairs {
                let pair = pair.as_array().expect("canonical header pair");
                let name = HeaderName::from_bytes(pair[0].as_str().unwrap().as_bytes())
                    .expect("canonical header name");
                let value = HeaderValue::from_bytes(pair[1].as_str().unwrap().as_bytes())
                    .expect("canonical header value");
                headers.append(name, value);
            }
        }
        (status, headers, upstream_body.clone())
    } else {
        (StatusCode::OK, HeaderMap::new(), Bytes::from_static(b"{}"))
    };

    // Bridge upstream-request artifacts are converter-layer contracts locked by
    // bridge_conversion_contract; production normalization sits on top, so skip egress matching.
    let expected_request = if case.is_bridge_direction() {
        None
    } else {
        case.expected_upstream_request.clone()
    };
    let state = MockState {
        expected_request,
        status,
        headers,
        body,
        observations: observations.clone(),
    };
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock upstream must bind loopback");
    let upstream_address = upstream_listener.local_addr().unwrap();
    let upstream_task = spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/chat/completions", post(mock_respond))
            .route("/v1/responses", post(mock_respond))
            .with_state(state),
    );

    // Start the production Router with the case-specific route shape.
    let GatewayHarness {
        address,
        task: gateway_task,
        metrics: _metrics,
    } = start_gateway_with_definition(upstream_address, replay_definition_for(case)).await;

    // Send the canonical downstream request through a real HTTP client.
    let response = reqwest::Client::new()
        .post(format!("http://{address}{}", downstream_endpoint(case)))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(case.client_request.to_vec())
        .send()
        .await
        .expect("replay request must complete");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    // Collect visible bytes, tolerating body transport errors after commit.
    let mut body = Vec::new();
    let mut body_error = false;
    let mut body_error_message = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => body.extend_from_slice(&chunk),
            Err(error) => {
                body_error = true;
                body_error_message = error.to_string();
                break;
            }
        }
    }

    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations.lock().await.clone();
    let attempts = upstream_request_matches.len();
    assert_eq!(
        is_reject,
        attempts == 0,
        "{}: reject cases must have zero upstream attempts",
        case.id
    );
    ReplayProbe {
        status,
        content_type,
        attempts,
        upstream_request_matches,
        body,
        body_error,
        body_error_message,
    }
}

/// Compares two SSE documents through the canonical Event IR semantic response.
pub fn assert_stream_semantic_eq(
    protocol: openbridge::core::ApiProtocol,
    actual: &[u8],
    expected: &[u8],
    context: &str,
) {
    use openbridge::bridge::StaticEventBridge;
    use openbridge::core::ReasoningOutput;
    use openbridge::ir::generation::EventLimits;
    use openbridge::transport::sse::SseDecoder;

    fn materialize(
        protocol: openbridge::core::ApiProtocol,
        document: &[u8],
    ) -> openbridge::ir::generation::GenerationResponse {
        let target = match protocol {
            openbridge::core::ApiProtocol::ChatCompletions => {
                openbridge::core::ApiProtocol::Responses
            }
            openbridge::core::ApiProtocol::Responses => {
                openbridge::core::ApiProtocol::ChatCompletions
            }
        };
        let reasoning = match protocol {
            openbridge::core::ApiProtocol::ChatCompletions => ReasoningOutput::PlainText,
            openbridge::core::ApiProtocol::Responses => ReasoningOutput::Summary,
        };
        let mut bridge = StaticEventBridge::new(
            protocol,
            target,
            "public-model",
            reasoning,
            false,
            EventLimits::new(256 * 1024, 1024 * 1024, 4 * 1024 * 1024).unwrap(),
        )
        .expect("semantic comparison bridge");
        let mut decoder = SseDecoder::new(256 * 1024);
        for event in decoder.push(document).expect("semantic comparison decode") {
            bridge.render(event).expect("semantic comparison stream");
        }
        for event in decoder.finish().expect("semantic comparison finish") {
            bridge.render(event).expect("semantic comparison stream");
        }
        bridge.finish().expect("semantic comparison terminal");
        bridge
            .materialized_response()
            .expect("semantic comparison response")
    }

    assert_eq!(
        materialize(protocol, actual),
        materialize(protocol, expected),
        "{context}"
    );
}
