//! Verifies bidirectional protocol conversion through the production Router on a Bridged Route.

mod support;

use std::{
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};

use axum::body::{Body, to_bytes};
use bytes::Bytes;
use futures_util::Stream;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Request, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    bridge::{ChatStreamState, ResponsesStreamState},
    core::{ApiProtocol, OperationKind, ReasoningOutput},
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::{
        ReasoningLevel, ReasoningProfile, RegistryError, RouteConfig, RouteMode, UpstreamTarget,
        build_registry,
    },
    transport::{
        sse::SseDecoder,
        upstream::{TransportError, UpstreamResponse, UpstreamTransport},
    },
};
use serde_json::Value;
use tower::ServiceExt;

struct ExpectedTransport {
    expected_path: &'static str,
    upstream_body: Bytes,
    content_type: &'static str,
    requests: Mutex<Vec<(String, Value)>>,
}

impl UpstreamTransport for ExpectedTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let path = request.relative_uri().path().to_owned();
            let body = serde_json::from_slice(request.body()).expect("upstream request JSON");
            self.requests.lock().unwrap().push((path, body));
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, self.content_type.parse().unwrap());
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(self.upstream_body.clone()),
            ))
        })
    }
}

struct PendingBridgeTransport {
    dropped: Arc<AtomicBool>,
}

impl UpstreamTransport for PendingBridgeTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, "text/event-stream".parse().unwrap());
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from_stream(PendingBridgeStream {
                    dropped: self.dropped.clone(),
                }),
            ))
        })
    }
}

struct PendingBridgeStream {
    dropped: Arc<AtomicBool>,
}

impl Stream for PendingBridgeStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}

impl Drop for PendingBridgeStream {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

fn fixture(path: &str) -> Bytes {
    Bytes::from(
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("testdata/cases/bridge")
                .join(path),
        )
        .expect("bridge fixture"),
    )
}

fn app(
    downstream: ApiProtocol,
    upstream: ApiProtocol,
    transport: Arc<dyn UpstreamTransport>,
) -> axum::Router {
    app_with_reasoning_output(downstream, upstream, transport, ReasoningOutput::Unknown)
}

fn app_with_reasoning_output(
    downstream: ApiProtocol,
    upstream: ApiProtocol,
    transport: Arc<dyn UpstreamTransport>,
    reasoning_output: ReasoningOutput,
) -> axum::Router {
    // Keep only the reverse Bridged Route so a Native candidate cannot mask conversion behavior.
    let mut definition = support::definition("bridge-forward", "public-model", "upstream-model");
    support::generation_profile_mut(&mut definition.models[0]).reasoning =
        ReasoningProfile::supported([ReasoningLevel::High]);
    let use_deepseek_chat =
        upstream == ApiProtocol::ChatCompletions && reasoning_output == ReasoningOutput::PlainText;
    if use_deepseek_chat {
        definition.credential_pools[0].id = "deepseek-primary".to_owned();
        definition.credential_pools[0].provider = openbridge::provider::ProviderKind::DeepSeek;
        let instance = &mut definition.provider_instances[0];
        instance.id = "deepseek-test".to_owned();
        instance.kind = openbridge::provider::ProviderKind::DeepSeek;
        instance.base_url = "https://api.deepseek.com".to_owned();
        let target = &mut definition.upstream_targets[0];
        target.provider_instance = "deepseek-test".to_owned();
        target.provider_model = "deepseek/test-model".to_owned();
        target.credential_pool = "deepseek-primary".to_owned();
        target
            .upstream_apis
            .retain(|api| api.capabilities.operation() == OperationKind::ChatCompletions);
    }
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = capabilities.function_tools.map(|mut profile| {
            profile.parallel_calls = !use_deepseek_chat;
            profile
        });
        capabilities.reasoning_output = reasoning_output;
    }
    if !use_deepseek_chat
        && let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.function_tools = capabilities.function_tools.map(|mut profile| {
            profile.parallel_calls = true;
            profile
        });
        capabilities.reasoning_output = reasoning_output;
    }
    definition.routes = vec![RouteConfig {
        id: "bridge-route".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: upstream.operation(),
        downstream_operation: downstream.operation(),
        mode: RouteMode::Bridged,
    }];
    definition.public_models[0].routes = vec!["bridge-route".to_owned()];
    let registry = Arc::new(
        build_registry(support::bootstrap(support::BOOTSTRAP), definition)
            .expect("bridged registry must build"),
    );
    let (users, credentials) = support::users_and_credentials(
        "downstream-token-0000000000000000",
        &registry,
        "upstream-token",
    );
    build_router(GatewayState::new(registry, transport, users, credentials))
}

fn assert_stream_semantics(protocol: ApiProtocol, actual: &[u8], expected: &[u8]) {
    let decode = |body: &[u8]| {
        let mut decoder = SseDecoder::new(256 * 1024);
        let mut events = decoder.push(body).unwrap();
        events.extend(decoder.finish().unwrap());
        events
    };
    match protocol {
        ApiProtocol::ChatCompletions => {
            let mut actual_state = ChatStreamState::new();
            for event in decode(actual) {
                actual_state.ingest(&event).unwrap();
            }
            actual_state.finish().unwrap();
            let mut expected_state = ChatStreamState::new();
            for event in decode(expected) {
                expected_state.ingest(&event).unwrap();
            }
            expected_state.finish().unwrap();
            assert_eq!(actual_state.text(), expected_state.text());
            assert_eq!(
                actual_state.reasoning_text(),
                expected_state.reasoning_text()
            );
            assert_eq!(actual_state.tool_calls(), expected_state.tool_calls());
        }
        ApiProtocol::Responses => {
            let mut actual_state = ResponsesStreamState::new();
            for event in decode(actual) {
                actual_state.ingest(&event).unwrap();
            }
            actual_state.finish().unwrap();
            let mut expected_state = ResponsesStreamState::new();
            for event in decode(expected) {
                expected_state.ingest(&event).unwrap();
            }
            expected_state.finish().unwrap();
            assert_eq!(actual_state.text(), expected_state.text());
            assert_eq!(
                actual_state.reasoning_text(),
                expected_state.reasoning_text()
            );
            assert_eq!(actual_state.tool_calls(), expected_state.tool_calls());
        }
    }
}

#[tokio::test]
async fn production_router_converts_non_stream_requests_and_responses_in_both_directions() {
    let cases = [
        (
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "/v1/chat/completions",
            "/v1/responses",
            "chat_to_responses/chat_to_responses.text.non_stream",
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "/v1/responses",
            "/v1/chat/completions",
            "responses_to_chat/responses_to_chat.text.non_stream",
        ),
    ];

    // Verify the production endpoint, request conversion, response conversion, and Public Model isolation in both directions.
    for (downstream, upstream, client_path, upstream_path, directory) in cases {
        let transport = Arc::new(ExpectedTransport {
            expected_path: upstream_path,
            upstream_body: fixture(&format!("{directory}/upstream-response.json")),
            content_type: "application/json",
            requests: Mutex::new(Vec::new()),
        });
        let app = app(downstream, upstream, transport.clone());
        let response = app
            .oneshot(
                Request::post(client_path)
                    .header(CONTENT_TYPE, "application/json")
                    .header("authorization", "Bearer downstream-token-0000000000000000")
                    .body(Body::from(fixture(&format!(
                        "{directory}/client-request.json"
                    ))))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let actual: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        let expected: Value = serde_json::from_slice(&fixture(&format!(
            "{directory}/expected-client-response.json"
        )))
        .unwrap();
        assert_eq!(actual, expected);

        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, transport.expected_path);
        let expected_upstream: Value = serde_json::from_slice(&fixture(&format!(
            "{directory}/expected-upstream-request.json"
        )))
        .unwrap();
        assert_eq!(requests[0].1, expected_upstream);
    }
}

#[tokio::test]
async fn production_router_converts_text_and_parallel_tool_streams_in_both_directions() {
    let cases = [
        (
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "/v1/chat/completions",
            "/v1/responses",
            "chat_to_responses/chat_to_responses.text.stream",
        ),
        (
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "/v1/chat/completions",
            "/v1/responses",
            "chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments",
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "/v1/responses",
            "/v1/chat/completions",
            "responses_to_chat/responses_to_chat.text.stream",
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "/v1/responses",
            "/v1/chat/completions",
            "responses_to_chat/responses_to_chat.parallel_tools.fragmented_arguments",
        ),
    ];

    // The production body stream must preserve semantics and one terminal while calling the reverse protocol endpoint.
    for (downstream, upstream, client_path, upstream_path, directory) in cases {
        let transport = Arc::new(ExpectedTransport {
            expected_path: upstream_path,
            upstream_body: fixture(&format!("{directory}/upstream-stream.sse")),
            content_type: "text/event-stream",
            requests: Mutex::new(Vec::new()),
        });
        let response = app(downstream, upstream, transport.clone())
            .oneshot(
                Request::post(client_path)
                    .header(CONTENT_TYPE, "application/json")
                    .header("authorization", "Bearer downstream-token-0000000000000000")
                    .body(Body::from(fixture(&format!(
                        "{directory}/client-request.json"
                    ))))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let actual = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_stream_semantics(
            downstream,
            &actual,
            &fixture(&format!("{directory}/expected-client-stream.sse")),
        );
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, upstream_path);
    }
}

#[tokio::test]
async fn production_router_keeps_reasoning_and_ignores_empty_chat_content_before_tool_terminal() {
    let upstream = Bytes::from_static(
        br#"data: {"id":"chatcmpl_mock_reasoning","choices":[{"delta":{"role":"assistant","content":""},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_mock_reasoning","choices":[{"delta":{"reasoning_content":"check args","content":""},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_mock_reasoning","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_lookup","function":{"name":"lookup","arguments":""}}]},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_mock_reasoning","choices":[{"delta":{"content":"","tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"Hangzhou\"}"}}]},"finish_reason":"tool_calls","index":0}]}

data: [DONE]

"#,
    );
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/chat/completions",
        upstream_body: upstream,
        content_type: "text/event-stream",
        requests: Mutex::new(Vec::new()),
    });
    let response = app_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport.clone(),
        ReasoningOutput::PlainText,
    )
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","input":"hello","stream":true,"reasoning":{"effort":"high"},"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("response.output_text"));

    // Use the production HTTP stream state machine to confirm closed reasoning, tool arguments, and terminal state.
    let mut state = ResponsesStreamState::new();
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    for event in events {
        state.ingest(&event).unwrap();
    }
    state.finish().unwrap();
    assert_eq!(state.reasoning_text(), "check args");
    assert_eq!(state.tool_calls().len(), 1);
    assert_eq!(
        state.terminal(),
        Some(openbridge::bridge::StreamTerminal::Completed)
    );

    // The mock upstream must receive the explicit standard Responses reasoning-to-Chat effort mapping.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "/chat/completions");
    assert_eq!(requests[0].1["reasoning_effort"], "high");
}

#[tokio::test]
async fn production_router_accepts_post_finish_chat_usage_chunk() {
    let upstream = Bytes::from_static(
        br#"data: {"id":"chatcmpl_mock_usage","choices":[{"delta":{"content":"ok"},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_mock_usage","choices":[{"delta":{},"finish_reason":"stop","index":0}]}

data: {"id":"chatcmpl_mock_usage","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}

data: [DONE]

"#,
    );
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/chat/completions",
        upstream_body: upstream,
        content_type: "text/event-stream",
        requests: Mutex::new(Vec::new()),
    });

    // Send a streaming Responses request through the production router and collect the converted body.
    let response = app_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport,
        ReasoningOutput::PlainText,
    )
    .oneshot(
        Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header("authorization", "Bearer downstream-token-0000000000000000")
            .body(Body::from(
                r#"{"model":"public-model","input":"hello","stream":true}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();

    // Verify that the ignored usage chunk does not prevent a valid Responses terminal.
    let mut state = ResponsesStreamState::new();
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    for event in events {
        state.ingest(&event).unwrap();
    }
    state.finish().unwrap();
    assert_eq!(state.text(), "ok");
    assert_eq!(
        state.terminal(),
        Some(openbridge::bridge::StreamTerminal::Completed)
    );
}

#[tokio::test]
async fn production_router_rejects_reasoning_when_upstream_output_is_unknown() {
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/chat/completions",
        upstream_body: Bytes::new(),
        content_type: "application/json",
        requests: Mutex::new(Vec::new()),
    });
    let response = app(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport.clone(),
    )
    .oneshot(
        Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header("authorization", "Bearer downstream-token-0000000000000000")
            .body(Body::from(
                r#"{"model":"public-model","input":"hello","reasoning":{"effort":"high"}}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn production_router_rejects_unbridgeable_requests_before_egress() {
    let directories = [
        "responses_to_chat/responses_to_chat.continuation.reject",
        "responses_to_chat/responses_to_chat.unsupported_hosted_tool.reject",
        "responses_to_chat/responses_to_chat.duplicate_call_id.reject",
        "responses_to_chat/responses_to_chat.empty_arguments.reject",
        "responses_to_chat/responses_to_chat.unknown_tool_result.reject",
    ];
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/chat/completions",
        upstream_body: Bytes::new(),
        content_type: "application/json",
        requests: Mutex::new(Vec::new()),
    });
    let app = app(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport.clone(),
    );

    // Bridge preflight must reject every canonical reject case before the credential or transport boundary.
    for directory in directories {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .header("authorization", "Bearer downstream-token-0000000000000000")
                    .body(Body::from(fixture(&format!(
                        "{directory}/client-request.json"
                    ))))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{directory}");
        if directory.ends_with("unsupported_hosted_tool.reject") {
            let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
            let error: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(error["error"]["code"], "unimplemented_request");
        }
    }

    // Reject a known Native image part because a protocol Bridge contributes no image source.
    let image_response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","input":[{"role":"user","content":[{"type":"input_image","image_url":"https://example.invalid/image.png"}]}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(image_response.status(), StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_slice(
        &to_bytes(image_response.into_body(), 64 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["error"]["code"], "unsupported_model_capability");

    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn registry_requires_bridged_routes_to_target_the_opposite_protocol() {
    let mut definition = support::definition("bridge-invalid", "public-model", "upstream-model");
    definition.routes[0].mode = RouteMode::Bridged;

    let error = build_registry(support::bootstrap(support::BOOTSTRAP), definition)
        .expect_err("same-protocol Bridged Route must fail at startup");
    assert!(matches!(
        error,
        RegistryError::InvalidBridgedRouteOperations { route } if route == "public-chat"
    ));
}

#[tokio::test]
async fn invalid_bridged_stream_closes_without_fabricating_a_terminal() {
    let directory = "responses_to_chat/responses_to_chat.incomplete_arguments.stream";
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/chat/completions",
        upstream_body: fixture(&format!("{directory}/upstream-stream.sse")),
        content_type: "text/event-stream",
        requests: Mutex::new(Vec::new()),
    });
    let response = app(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport.clone(),
    )
    .oneshot(
        Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header("authorization", "Bearer downstream-token-0000000000000000")
            .body(Body::from(fixture(&format!(
                "{directory}/client-request.json"
            ))))
            .unwrap(),
    )
    .await
    .unwrap();

    // After HTTP commitment, the body may end with an error but cannot fabricate response.completed or fallback.
    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 1024 * 1024).await.is_err());
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn bridged_stream_requires_an_upstream_sse_response() {
    let directory = "responses_to_chat/responses_to_chat.text.stream";
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/chat/completions",
        upstream_body: Bytes::from_static(br#"{"id":"unexpected-json"}"#),
        content_type: "application/json",
        requests: Mutex::new(Vec::new()),
    });
    let response = app(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport,
    )
    .oneshot(
        Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header("authorization", "Bearer downstream-token-0000000000000000")
            .body(Body::from(fixture(&format!(
                "{directory}/client-request.json"
            ))))
            .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn dropping_a_bridged_downstream_body_cancels_the_upstream_stream() {
    let dropped = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(PendingBridgeTransport {
        dropped: dropped.clone(),
    });
    let app = app(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        transport,
    );
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","input":"hello","stream":true}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    assert!(dropped.load(Ordering::SeqCst));
}
