//! 验证 Bridged Route 通过生产 Router 执行双向协议转换。

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
    core::ApiProtocol,
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::{RegistryError, RouteConfig, RouteMode, UpstreamTarget, build_registry},
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
    // 只保留一条方向相反的 Bridged Route，确保 Native 候选不能掩盖转换行为。
    let mut definition = support::definition("bridge-forward", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.parallel_tool_calls = true;
    }
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.parallel_tool_calls = true;
    }
    definition.routes = vec![RouteConfig {
        id: "bridge-route".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_api: if upstream == ApiProtocol::Responses {
            "responses".to_owned()
        } else {
            "chat".to_owned()
        },
        downstream_protocol: downstream,
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

    // 逐方向验证生产路径的 endpoint、请求转换、响应转换和 Public Model 隔离。
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

    // 生产 Body stream 必须保持语义和唯一 terminal，同时实际调用相反协议 endpoint。
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

    // Bridge preflight 必须在 credential/transport 边界前拒绝全部 canonical reject cases。
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
    }
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
        RegistryError::BridgedRouteProtocolMatch { route } if route == "public-chat"
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

    // HTTP 已提交后只能让 body 以错误结束，不能补造 response.completed 或 fallback。
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
