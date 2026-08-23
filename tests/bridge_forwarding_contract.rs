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
use futures_util::future::BoxFuture;
use futures_util::{Stream, StreamExt};
use http::{HeaderMap, Request, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    bridge::{ChatStreamState, ResponsesStreamState},
    core::{
        ALL_TOOL_CHOICE_MODES, ApiProtocol, FunctionToolCapabilities, GenerationBridgeDirection,
        OperationKind, ReasoningOutput,
    },
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

struct RetryThenSuccessTransport {
    upstream_body: Bytes,
    attempts: Mutex<Vec<Value>>,
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

impl UpstreamTransport for RetryThenSuccessTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let body = serde_json::from_slice(request.body()).expect("upstream request JSON");
        let mut attempts = self.attempts.lock().unwrap();
        attempts.push(body);
        let should_retry = attempts.len() == 1;
        drop(attempts);
        Box::pin(async move {
            if should_retry {
                return Err(TransportError::Timeout);
            }
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, "text/event-stream".parse().unwrap());
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
                    sent_first: false,
                }),
            ))
        })
    }
}

struct PendingBridgeStream {
    dropped: Arc<AtomicBool>,
    sent_first: bool,
}

impl Stream for PendingBridgeStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.sent_first {
            self.sent_first = true;
            return Poll::Ready(Some(Ok(Bytes::from_static(
                b"data: {\"id\":\"chatcmpl_pending\",\"object\":\"chat.completion.chunk\",\"model\":\"upstream-model\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"hi\"},\"finish_reason\":null}]}\n\n",
            ))));
        }
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

fn responses_usage_stream() -> Bytes {
    Bytes::from_static(
        br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_router_usage","status":"in_progress"}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"msg_router_usage","type":"message","role":"assistant","status":"in_progress","content":[]}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_router_usage","output_index":0,"content_index":0,"delta":"ok"}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"id":"msg_router_usage","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"ok","annotations":[]}]}}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_router_usage","status":"completed","usage":{"input_tokens":5,"output_tokens":2,"total_tokens":7}}}

"#,
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
    app_with_protocol_capabilities(downstream, upstream, transport, reasoning_output, true)
}

fn app_with_protocol_capabilities(
    downstream: ApiProtocol,
    upstream: ApiProtocol,
    transport: Arc<dyn UpstreamTransport>,
    reasoning_output: ReasoningOutput,
    responses_terminal_usage: bool,
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
        capabilities.streaming = true;
        capabilities.stream_usage = true;
        capabilities.function_tools = Some(FunctionToolCapabilities {
            choice_modes: ALL_TOOL_CHOICE_MODES,
            parallel_calls: !use_deepseek_chat,
            strict_schema: false,
        });
        capabilities.reasoning_output = reasoning_output;
        capabilities.prompt_cache_key = false;
    }
    if !use_deepseek_chat
        && let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.streaming = true;
        capabilities.function_tools = Some(FunctionToolCapabilities {
            choice_modes: ALL_TOOL_CHOICE_MODES,
            parallel_calls: true,
            strict_schema: false,
        });
        capabilities.reasoning_output = reasoning_output;
        capabilities.terminal_usage = responses_terminal_usage;
    }
    definition.routes = vec![RouteConfig {
        id: "bridge-route".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: upstream.operation(),
        downstream_operation: downstream.operation(),
        mode: RouteMode::GenerationBridge(
            GenerationBridgeDirection::from_protocols(downstream, upstream)
                .expect("bridge harness requires distinct generation protocols"),
        ),
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
async fn responses_bridge_consumes_reasoning_include_before_chat_egress() {
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/chat/completions",
        upstream_body: Bytes::from_static(
            br#"{"id":"chatcmpl_include","object":"chat.completion","model":"upstream-model","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        content_type: "application/json",
        requests: Mutex::new(Vec::new()),
    });

    // Accept the known compatibility hint without promising or synthesizing opaque reasoning output.
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
                r#"{"model":"public-model","input":"hello","include":["reasoning.encrypted_content"]}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["output"][0]["content"][0]["text"], "ok");

    // The Chat wire has no include field, so the Bridge must consume it before egress.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "/chat/completions");
    assert!(requests[0].1.get("include").is_none());
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
        let mut expected_upstream: Value = serde_json::from_slice(&fixture(&format!(
            "{directory}/expected-upstream-request.json"
        )))
        .unwrap();
        match upstream {
            ApiProtocol::Responses => {
                expected_upstream["instructions"] = Value::String(
                    "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
                        .to_owned(),
                );
            }
            ApiProtocol::ChatCompletions => {
                expected_upstream["messages"]
                    .as_array_mut()
                    .unwrap()
                    .insert(
                        0,
                        serde_json::json!({
                            "role": "system",
                            "content": "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
                        }),
                    );
            }
        }
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
async fn production_router_fulfills_chat_stream_usage_through_responses() {
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/responses",
        upstream_body: responses_usage_stream(),
        content_type: "text/event-stream",
        requests: Mutex::new(Vec::new()),
    });
    let response = app(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        transport.clone(),
    )
    .oneshot(
        Request::post("/v1/chat/completions")
            .header(CONTENT_TYPE, "application/json")
            .header("authorization", "Bearer downstream-token-0000000000000000")
            .body(Body::from(
                r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true}}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    assert_eq!(events.last().map(|event| event.data()), Some("[DONE]"));
    let chunks = events[..events.len() - 1]
        .iter()
        .map(|event| serde_json::from_str::<Value>(event.data()).unwrap())
        .collect::<Vec<_>>();
    assert!(
        chunks[..chunks.len() - 1]
            .iter()
            .all(|chunk| chunk["usage"].is_null())
    );
    assert_eq!(chunks.last().unwrap()["choices"], serde_json::json!([]));
    assert_eq!(
        chunks.last().unwrap()["usage"],
        serde_json::json!({"prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7})
    );

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "/v1/responses");
    assert!(requests[0].1.get("stream_options").is_none());
}

#[tokio::test]
async fn retried_chat_usage_bridge_preserves_the_request_contract_until_success() {
    let transport = Arc::new(RetryThenSuccessTransport {
        upstream_body: responses_usage_stream(),
        attempts: Mutex::new(Vec::new()),
    });
    let response = app(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        transport.clone(),
    )
    .oneshot(
        Request::post("/v1/chat/completions")
            .header(CONTENT_TYPE, "application/json")
            .header("authorization", "Bearer downstream-token-0000000000000000")
            .body(Body::from(
                r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true}}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // The retry succeeds with one usage tail, proving the immutable Bridge output option survived.
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    assert_eq!(events.last().map(|event| event.data()), Some("[DONE]"));
    let usage_chunks = events
        .iter()
        .filter_map(|event| serde_json::from_str::<Value>(event.data()).ok())
        .filter(|chunk| chunk["choices"] == serde_json::json!([]))
        .collect::<Vec<_>>();
    assert_eq!(usage_chunks.len(), 1);
    assert_eq!(
        usage_chunks[0]["usage"],
        serde_json::json!({"prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7})
    );

    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    assert!(
        attempts.iter().all(|request| {
            request["stream"] == true && request.get("stream_options").is_none()
        })
    );
}

#[tokio::test]
async fn production_router_removes_chat_stream_usage_noops_before_any_egress() {
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/responses",
        upstream_body: fixture(
            "chat_to_responses/chat_to_responses.text.stream/upstream-stream.sse",
        ),
        content_type: "text/event-stream",
        requests: Mutex::new(Vec::new()),
    });
    let app = app(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        transport.clone(),
    );
    for stream_options in [
        serde_json::json!({}),
        serde_json::json!({"include_usage": false}),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json")
                    .header("authorization", "Bearer downstream-token-0000000000000000")
                    .body(Body::from(
                        serde_json::json!({
                            "model": "public-model",
                            "messages": [{"role": "user", "content": "hello"}],
                            "stream": true,
                            "stream_options": stream_options
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("\"usage\""));
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|(_, body)| body.get("stream_options").is_none())
    );
}

#[tokio::test]
async fn chat_usage_bridge_requires_terminal_usage_but_noops_do_not() {
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/responses",
        upstream_body: fixture(
            "chat_to_responses/chat_to_responses.text.stream/upstream-stream.sse",
        ),
        content_type: "text/event-stream",
        requests: Mutex::new(Vec::new()),
    });
    let app = app_with_protocol_capabilities(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        transport.clone(),
        ReasoningOutput::Unknown,
        false,
    );

    // The effective usage contract is rejected by Public Model preflight before any Provider attempt.
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "unsupported_model_capability");
    assert_eq!(error["error"]["param"], "stream_options");
    assert!(transport.requests.lock().unwrap().is_empty());

    // Omitted-equivalent shapes execute through the same fixed Bridge and are removed from egress.
    for stream_options in [
        serde_json::json!({}),
        serde_json::json!({"include_usage": false}),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json")
                    .header("authorization", "Bearer downstream-token-0000000000000000")
                    .body(Body::from(
                        serde_json::json!({
                            "model": "public-model",
                            "messages": [{"role": "user", "content": "hello"}],
                            "stream": true,
                            "stream_options": stream_options
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    }
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|(_, body)| body.get("stream_options").is_none())
    );
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
    for summary in [serde_json::json!(false), serde_json::json!("auto")] {
        let transport = Arc::new(ExpectedTransport {
            expected_path: "/chat/completions",
            upstream_body: upstream.clone(),
            content_type: "text/event-stream",
            requests: Mutex::new(Vec::new()),
        });
        let request = serde_json::json!({
            "model": "public-model",
            "input": "hello",
            "stream": true,
            "reasoning": {"effort": "high", "summary": summary},
            "tools": [{"type": "function", "name": "lookup", "parameters": {"type": "object"}}]
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
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let wire = String::from_utf8_lossy(&body);
        assert!(!wire.contains("response.output_text"));
        assert!(!wire.contains("response.reasoning_summary_"));

        // Confirm that Chat reasoning remains Responses reasoning content for either accepted summary hint.
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

        // The Chat wire receives only the mapped effort, never a fabricated summary field.
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "/chat/completions");
        assert_eq!(requests[0].1["reasoning_effort"], "high");
        assert!(requests[0].1.get("reasoning").is_none());
        assert!(requests[0].1.get("summary").is_none());
    }
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
fn registry_requires_bridge_operations_to_match_the_declared_direction() {
    let mut definition = support::definition("bridge-invalid", "public-model", "upstream-model");
    definition.routes[0].mode =
        RouteMode::GenerationBridge(GenerationBridgeDirection::ChatToResponses);

    let error = build_registry(support::bootstrap(support::BOOTSTRAP), definition)
        .expect_err("direction-mismatched Generation Bridge Route must fail at startup");
    assert!(matches!(
        error,
        RegistryError::InvalidGenerationBridgeRoute { route } if route == "public-chat"
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
    let mut body = response.into_body().into_data_stream();
    let mut partial = Vec::new();
    let mut body_error = false;
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(chunk) => partial.extend_from_slice(&chunk),
            Err(_) => {
                body_error = true;
                break;
            }
        }
    }
    assert!(body_error);
    assert!(
        !partial.is_empty(),
        "valid converted prefix must remain visible"
    );
    let partial = std::str::from_utf8(&partial).unwrap();
    assert!(!partial.contains("response.completed"));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn same_chunk_bridge_error_preserves_every_prior_converted_event() {
    let upstream_body = Bytes::from_static(
        br#"data: {"id":"chatcmpl_partial","object":"chat.completion.chunk","model":"upstream-model","choices":[{"index":0,"delta":{"role":"assistant","content":"A"},"finish_reason":null}]}

data: {"id":"chatcmpl_partial","object":"chat.completion.chunk","model":"upstream-model","choices":[{"index":0,"delta":{"content":"B"},"finish_reason":null}]}

data: {not-json}

"#,
    );
    let transport = Arc::new(ExpectedTransport {
        expected_path: "/v1/chat/completions",
        upstream_body,
        content_type: "text/event-stream",
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
            .body(Body::from(
                r#"{"model":"public-model","input":"hello","stream":true}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    let mut partial = Vec::new();
    let mut body_error = false;
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(chunk) => partial.extend_from_slice(&chunk),
            Err(_) => {
                body_error = true;
                break;
            }
        }
    }
    assert!(body_error);
    let partial = std::str::from_utf8(&partial).unwrap();
    assert!(partial.contains("A"));
    assert!(partial.contains("B"));
    assert!(!partial.contains("response.completed"));
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
