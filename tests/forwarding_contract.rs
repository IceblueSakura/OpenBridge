mod support;

use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, SET_COOKIE, USER_AGENT},
    },
};
use bytes::Bytes;
use futures_util::{future::BoxFuture, stream};
use http::{HeaderMap, HeaderValue};
use openbridge::{
    core::ApiProtocol,
    ingress::{GatewayState, build_router},
    provider::{PreparedUpstreamRequest, ProviderKind},
    registry::{
        ReasoningLevel, ReasoningLevelMapping, ReasoningSupport, RegistryConfig, RouteConfig,
        RouteMode, UpstreamTarget, build_registry,
    },
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::Value;
use tower::ServiceExt;

#[derive(Debug)]
struct RecordedRequest {
    path: String,
    authorization: String,
    user_agent: Option<String>,
    body: Value,
}

#[derive(Default)]
struct RecordingTransport {
    requests: Mutex<Vec<RecordedRequest>>,
}

struct TimeoutTransport;

struct NonSseErrorTransport;

#[derive(Default)]
struct RateLimitedTransport {
    attempts: Mutex<usize>,
}

struct InvalidSseTransport;

struct EofWithoutTerminalTransport;

struct PartialStreamFailureTransport {
    attempts: AtomicUsize,
}

struct PendingSseTransport {
    dropped: Arc<AtomicBool>,
}

struct DropSignal(Arc<AtomicBool>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct FailoverTransport {
    attempted_models: Mutex<Vec<String>>,
}

#[derive(Default)]
struct BoundedFailoverTransport {
    attempts: Mutex<Vec<(String, ProviderKind, Instant)>>,
}

struct PendingRequestTransport {
    attempts: AtomicUsize,
    started: tokio::sync::Notify,
    dropped: Arc<AtomicBool>,
}

struct BackoffCancellationTransport {
    attempts: AtomicUsize,
    first_attempt: tokio::sync::Notify,
}

#[derive(Default)]
struct ScopedHealthTransport {
    attempts: Mutex<Vec<String>>,
}

#[derive(Default)]
struct ScopedFaultTransport {
    attempts: Mutex<Vec<String>>,
}

impl UpstreamTransport for TimeoutTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async { Err(TransportError::Timeout) })
    }
}

impl UpstreamTransport for PendingRequestTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // 记录已开始的 pending request 并通知测试任务。
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.started.notify_one();
        let signal = DropSignal(self.dropped.clone());

        // 保持上游 future pending，以观察下游取消是否向上传播析构。
        Box::pin(async move {
            let _signal = signal;
            std::future::pending::<Result<UpstreamResponse, TransportError>>().await
        })
    }
}

impl UpstreamTransport for BoundedFailoverTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // 记录候选、Provider 与开始时间，供预算和退避断言使用。
        let target_id = target.credential().id().to_owned();
        let provider = target.kind();
        self.attempts
            .lock()
            .unwrap()
            .push((target_id.clone(), provider, Instant::now()));
        // 按 Provider 返回不同的 retryable HTTP failure。
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let status = if provider == ProviderKind::LongCat {
                headers.insert("retry-after", HeaderValue::from_static("3"));
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            Ok(UpstreamResponse::new(
                status,
                headers,
                Body::from(format!(r#"{{"error":{{"message":"{target_id} failed"}}}}"#)),
            ))
        })
    }
}

impl UpstreamTransport for BackoffCancellationTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // 记录首次 attempt 并唤醒等待取消请求的测试任务。
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt == 1 {
            self.first_attempt.notify_one();
        }
        // 返回 retryable failure，使 handler 进入可取消的退避等待。
        Box::pin(async {
            Ok(UpstreamResponse::new(
                StatusCode::SERVICE_UNAVAILABLE,
                HeaderMap::new(),
                Body::from(r#"{"error":{"message":"temporary failure"}}"#),
            ))
        })
    }
}

impl UpstreamTransport for ScopedHealthTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // 使用 credential binding id 区分测试 target，并记录跨请求调用顺序。
        let target_id = target.credential().id().to_owned();
        self.attempts.lock().unwrap().push(target_id.clone());

        // 主目标返回带 cooldown 建议的 429，其余目标稳定成功。
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if target_id == "openai-primary" {
                headers.insert("retry-after", HeaderValue::from_static("10"));
                Ok(UpstreamResponse::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    headers,
                    Body::from(r#"{"error":{"message":"shared quota exhausted"}}"#),
                ))
            } else {
                Ok(UpstreamResponse::new(
                    StatusCode::OK,
                    headers,
                    Body::from(r#"{"id":"healthy-response"}"#),
                ))
            }
        })
    }
}

impl UpstreamTransport for ScopedFaultTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // 记录 target 顺序，并让主目标产生可重试 transport failure。
        let target_id = target.credential().id().to_owned();
        self.attempts.lock().unwrap().push(target_id.clone());
        Box::pin(async move {
            if target_id == "openai-primary" {
                Err(TransportError::Timeout)
            } else {
                Ok(UpstreamResponse::new(
                    StatusCode::OK,
                    HeaderMap::new(),
                    Body::from(r#"{"id":"healthy-response"}"#),
                ))
            }
        })
    }
}

impl UpstreamTransport for NonSseErrorTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::BAD_REQUEST,
                response_headers,
                Body::from(Bytes::from_static(b"\xff")),
            ))
        })
    }
}

impl UpstreamTransport for RateLimitedTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        *self.attempts.lock().unwrap() += 1;
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            response_headers.insert("retry-after", HeaderValue::from_static("2"));
            response_headers.insert("x-should-retry", HeaderValue::from_static("true"));
            Ok(UpstreamResponse::new(
                StatusCode::TOO_MANY_REQUESTS,
                response_headers,
                Body::from(r#"{"error":{"message":"rate limited"}}"#),
            ))
        })
    }
}

impl UpstreamTransport for InvalidSseTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(vec![Ok::<_, Infallible>(Bytes::from_static(
                    b"data: \xff\n\n",
                ))])),
            ))
        })
    }
}

impl UpstreamTransport for EofWithoutTerminalTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from(Bytes::from_static(
                    b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"sequence_number\":1,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"hi\",\"logprobs\":[]}\n\n",
                )),
            ))
        })
    }
}

impl UpstreamTransport for PartialStreamFailureTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            let event = b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"sequence_number\":1,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"hi\",\"logprobs\":[]}\n\n";
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(vec![
                    Ok::<_, std::io::Error>(Bytes::from_static(event)),
                    Err(std::io::Error::other("upstream connection reset")),
                ])),
            ))
        })
    }
}

impl UpstreamTransport for PendingSseTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let signal = DropSignal(self.dropped.clone());
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            let body = Body::from_stream(stream::once(async move {
                let _signal = signal;
                std::future::pending::<Result<Bytes, Infallible>>().await
            }));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                body,
            ))
        })
    }
}

impl UpstreamTransport for RecordingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let path = request.relative_uri().path().to_owned();
        self.requests.lock().unwrap().push(RecordedRequest {
            path: path.clone(),
            authorization: headers[AUTHORIZATION].to_str().unwrap().to_owned(),
            user_agent: headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: serde_json::from_slice(request.body()).unwrap(),
        });
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static(if path.ends_with("responses") {
                    "text/event-stream"
                } else {
                    "application/json"
                }),
            );
            response_headers.insert("openai-request-id", HeaderValue::from_static("upstream-id"));
            response_headers.insert(SET_COOKIE, HeaderValue::from_static("must-not-pass=true"));
            let chunks = if path.ends_with("responses") {
                vec![
                    Ok::<_, Infallible>(Bytes::from_static(b"event: response.output_text.delta\n")),
                    Ok(Bytes::from_static(b"data: {\"delta\":\"hi\"}\n\n")),
                ]
            } else {
                vec![Ok(Bytes::from_static(b"{\"id\":\"chat-result\"}"))]
            };
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(chunks)),
            ))
        })
    }
}

impl UpstreamTransport for FailoverTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let model = serde_json::from_slice::<Value>(request.body()).unwrap()["model"]
            .as_str()
            .unwrap()
            .to_owned();
        self.attempted_models.lock().unwrap().push(model.clone());
        Box::pin(async move {
            let status = if model == "fallback-model" {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            Ok(UpstreamResponse::new(
                status,
                HeaderMap::new(),
                Body::from("{}"),
            ))
        })
    }
}

fn app_with_transport(transport: Arc<dyn UpstreamTransport>) -> axum::Router {
    app_with_transport_and_definition(
        transport,
        support::definition("forward-test", "public-model", "upstream-model"),
    )
}

fn app_with_transport_and_definition(
    transport: Arc<dyn UpstreamTransport>,
    definition: RegistryConfig,
) -> axum::Router {
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();
    let (users, credentials) = support::users_and_credentials(
        "downstream-token-0000000000000000",
        &registry,
        "upstream-token",
    );
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials);
    build_router(state)
}

fn add_responses_fallback(
    definition: &mut RegistryConfig,
    target_id: &str,
    provider: ProviderKind,
) {
    // 从主目标复制同一模型事实，并切换到指定 Provider 的受信 endpoint profile。
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = target_id.to_owned();
    fallback.provider = provider;
    fallback.base_url = match provider {
        ProviderKind::OpenAi => "https://api.openai.com".to_owned(),
        ProviderKind::LongCat => "https://api.longcat.chat".to_owned(),
        ProviderKind::DeepSeek | ProviderKind::MiMo => {
            panic!("test fallback helper only accepts connected providers")
        }
    };
    fallback.credential.id = format!("{target_id}-credential");
    for upstream_api in &mut fallback.upstream_apis {
        upstream_api.endpoint_profile = match provider {
            ProviderKind::OpenAi => "public-api".to_owned(),
            ProviderKind::LongCat => "longcat-openai".to_owned(),
            ProviderKind::DeepSeek | ProviderKind::MiMo => {
                panic!("test fallback helper only accepts connected providers")
            }
        };
    }
    definition.upstream_targets.push(fallback);

    // 将新目标注册为同一 Public Model 的完整 Responses Route。
    let route_id = format!("{target_id}-responses");
    definition.routes.push(RouteConfig {
        id: route_id.clone(),
        upstream_target: target_id.to_owned(),
        upstream_api: "responses".to_owned(),
        downstream_protocol: ApiProtocol::Responses,
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes.push(route_id);
}

#[tokio::test]
async fn business_endpoints_reject_unauthenticated_requests_before_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(transport.requests.lock().unwrap().is_empty());
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_api_key");
}

#[tokio::test]
async fn business_endpoints_require_json_content_type_before_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());

    for content_type in [None, Some("text/plain")] {
        let mut request = Request::post("/v1/chat/completions")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000");
        if let Some(content_type) = content_type {
            request = request.header(CONTENT_TYPE, content_type);
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn provider_request_header_hook_overrides_user_agent_for_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    for path in ["/v1/chat/completions", "/v1/responses"] {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .header(USER_AGENT, "openbridge-contract-client/1.0")
            .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert_eq!(
            request.user_agent.as_deref(),
            Some("openbridge-contract-client/1.0")
        );
        assert_eq!(request.authorization, "Bearer upstream-token");
    }
}

#[tokio::test]
async fn models_lists_only_public_models_after_authentication() {
    let app = app_with_transport(Arc::new(RecordingTransport::default()));
    let request = Request::get("/v1/models")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let models: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(models["object"], "list");
    assert_eq!(
        models["data"],
        serde_json::json!([
            {"id": "public-model", "object": "model", "owned_by": "openbridge"}
        ])
    );
    assert!(!std::str::from_utf8(&body).unwrap().contains("openai-main"));
}

#[tokio::test]
async fn streaming_requests_fail_over_to_the_next_compatible_target_before_output() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = "openai-fallback".to_owned();
    fallback.credential.id = "openai-fallback-credential".to_owned();
    fallback.upstream_apis[1].upstream_model = "fallback-model".to_owned();
    definition.upstream_targets.push(fallback);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "fallback-responses".to_owned(),
        upstream_target: "openai-fallback".to_owned(),
        upstream_api: "responses".to_owned(),
        downstream_protocol: openbridge::core::ApiProtocol::Responses,
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("fallback-responses".to_owned());
    let transport = Arc::new(FailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let attempted_models = transport.attempted_models.lock().unwrap();
    assert_eq!(attempted_models.last(), Some(&"fallback-model".to_owned()));
    assert!(
        attempted_models
            .iter()
            .any(|model| model == "upstream-model")
    );
}

#[tokio::test]
async fn transient_failures_back_off_and_fall_back_to_another_provider_with_final_error() {
    // 构造 OpenAI 主目标与 LongCat fallback，并让两者都返回 transient failure。
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    add_responses_fallback(&mut definition, "longcat-fallback", ProviderKind::LongCat);
    let transport = Arc::new(BoundedFailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let started = Instant::now();

    // 执行请求并等待有限 retry/fallback 生命周期收敛。
    let response = app.oneshot(request).await.unwrap();

    // 验证指数退避、跨 Provider 顺序和最后一个安全 HTTP 错误。
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "3");
    assert!(started.elapsed() >= Duration::from_millis(300));
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(
        body,
        r#"{"error":{"message":"longcat-fallback-credential failed"}}"#
    );
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(
        attempts
            .iter()
            .map(|(target, provider, _)| (target.as_str(), *provider))
            .collect::<Vec<_>>(),
        vec![
            ("openai-primary", ProviderKind::OpenAi),
            ("openai-primary", ProviderKind::OpenAi),
            ("longcat-fallback-credential", ProviderKind::LongCat),
            ("longcat-fallback-credential", ProviderKind::LongCat),
        ]
    );
}

#[tokio::test]
async fn cross_request_health_skips_all_targets_in_the_cooled_quota_scope() {
    // 构造两个共享 quota scope 的目标和一个独立健康目标。
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.upstream_targets[0].quota_scope = Some("shared-quota".to_owned());
    add_responses_fallback(&mut definition, "shared-quota-peer", ProviderKind::OpenAi);
    definition.upstream_targets[1].quota_scope = Some("shared-quota".to_owned());
    add_responses_fallback(&mut definition, "independent-target", ProviderKind::OpenAi);
    definition.upstream_targets[2].quota_scope = Some("independent-quota".to_owned());
    let transport = Arc::new(ScopedHealthTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // 首个请求触发主目标 429；共享 scope peer 必须被跳过，独立目标完成请求。
    for _ in 0..2 {
        let request = Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // 第二个无状态请求必须直接复用独立目标，不再冲击已知受限 scope。
    assert_eq!(
        transport.attempts.lock().unwrap().as_slice(),
        [
            "openai-primary",
            "openai-primary",
            "independent-target-credential",
            "independent-target-credential",
        ]
    );
}

#[tokio::test]
async fn cross_request_health_skips_all_targets_in_the_cooled_fault_domain() {
    // 构造两个共享 fault domain 的目标和一个独立故障边界。
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.upstream_targets[0].fault_domain = Some("shared-fault".to_owned());
    add_responses_fallback(&mut definition, "shared-fault-peer", ProviderKind::OpenAi);
    definition.upstream_targets[1].fault_domain = Some("shared-fault".to_owned());
    add_responses_fallback(&mut definition, "independent-target", ProviderKind::OpenAi);
    definition.upstream_targets[2].fault_domain = Some("independent-fault".to_owned());
    let transport = Arc::new(ScopedFaultTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // 连续两个请求都应在主目标首次失败后选择独立 fault domain。
    for _ in 0..2 {
        let request = Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    assert_eq!(
        transport.attempts.lock().unwrap().as_slice(),
        [
            "openai-primary",
            "openai-primary",
            "independent-target-credential",
            "independent-target-credential",
        ]
    );
}

#[tokio::test]
async fn target_bound_continuation_ignores_cooldown_without_cross_target_fallback() {
    // 为两个 Responses API 开启 continuation，并让主目标先进入 quota cooldown。
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.previous_response_id = true;
    }
    add_responses_fallback(&mut definition, "healthy-fallback", ProviderKind::OpenAi);
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[1].upstream_apis[1].capabilities
    {
        capabilities.previous_response_id = true;
    }
    let transport = Arc::new(ScopedHealthTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // 无状态请求使主目标 cooldown，并安全 fallback 到第二目标。
    let warmup = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(warmup).await.unwrap().status(),
        StatusCode::OK
    );

    // continuation 必须继续尝试原 target，不能因 cooldown 静默切换到 fallback。
    let continuation = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","previous_response_id":"resp_123"}"#,
        ))
        .unwrap();
    let response = app.oneshot(continuation).await.unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        transport.attempts.lock().unwrap().as_slice(),
        [
            "openai-primary",
            "openai-primary",
            "healthy-fallback-credential",
            "openai-primary",
            "openai-primary",
        ]
    );
}

#[tokio::test]
async fn non_streaming_transient_failures_use_the_same_finite_retry_policy() {
    // 构造只含一个失败目标的非流式 Chat 请求。
    let transport = Arc::new(BoundedFailoverTransport::default());
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .unwrap();
    let started = Instant::now();

    // 执行请求并等待同候选的有限退避重试。
    let response = app.oneshot(request).await.unwrap();

    // 验证非流式路径也使用相同策略且不会超过候选局部上限。
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(started.elapsed() >= Duration::from_millis(50));
    assert_eq!(transport.attempts.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn request_attempt_budget_is_global_and_reserves_untried_fallbacks() {
    // 构造四个全部失败的有序候选，超过逐候选两次所需的请求总预算。
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    add_responses_fallback(&mut definition, "longcat-second", ProviderKind::LongCat);
    add_responses_fallback(&mut definition, "openai-third", ProviderKind::OpenAi);
    add_responses_fallback(&mut definition, "longcat-fourth", ProviderKind::LongCat);
    let transport = Arc::new(BoundedFailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    // 执行请求直到 request-wide budget 或候选收敛。
    let response = app.oneshot(request).await.unwrap();

    // 验证六次硬上限仍覆盖全部四个候选，并返回最后候选的错误。
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(
        body,
        r#"{"error":{"message":"longcat-fourth-credential failed"}}"#
    );
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(
        attempts
            .iter()
            .map(|(target, _, _)| target.as_str())
            .collect::<Vec<_>>(),
        vec![
            "openai-primary",
            "openai-primary",
            "longcat-second-credential",
            "longcat-second-credential",
            "openai-third-credential",
            "longcat-fourth-credential",
        ]
    );
}

#[tokio::test]
async fn provider_bound_streams_do_not_fall_back_to_another_target() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.previous_response_id = true;
    }
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = "openai-fallback".to_owned();
    fallback.credential.id = "openai-fallback-credential".to_owned();
    fallback.upstream_apis[1].upstream_model = "fallback-model".to_owned();
    definition.upstream_targets.push(fallback);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "fallback-responses".to_owned(),
        upstream_target: "openai-fallback".to_owned(),
        upstream_api: "responses".to_owned(),
        downstream_protocol: openbridge::core::ApiProtocol::Responses,
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("fallback-responses".to_owned());
    let transport = Arc::new(FailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true,"previous_response_id":"resp_123"}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        transport.attempted_models.lock().unwrap().as_slice(),
        ["upstream-model", "upstream-model"]
    );
}

#[tokio::test]
async fn dropping_the_downstream_stream_cancels_the_pending_upstream_stream() {
    let dropped = Arc::new(AtomicBool::new(false));
    let app = app_with_transport(Arc::new(PendingSseTransport {
        dropped: dropped.clone(),
    }));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn aborting_downstream_before_response_cancels_the_pending_upstream_request() {
    // 构造在 response headers 前永久 pending 的上游请求。
    let dropped = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(PendingRequestTransport {
        attempts: AtomicUsize::new(0),
        started: tokio::sync::Notify::new(),
        dropped: dropped.clone(),
    });
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let task = tokio::spawn(app.oneshot(request));
    transport.started.notified().await;

    // 模拟下游断开并等待 handler future 完成取消。
    task.abort();
    let error = task.await.unwrap_err();

    // 验证 pending send 已析构且没有启动第二次 attempt。
    assert!(error.is_cancelled());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn aborting_downstream_during_backoff_prevents_the_next_attempt() {
    // 构造首次即失败并进入退避的上游请求。
    let transport = Arc::new(BackoffCancellationTransport {
        attempts: AtomicUsize::new(0),
        first_attempt: tokio::sync::Notify::new(),
    });
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let task = tokio::spawn(app.oneshot(request));
    transport.first_attempt.notified().await;

    // 在退避 timer 完成前模拟下游断开。
    task.abort();
    let error = task.await.unwrap_err();
    tokio::time::sleep(Duration::from_millis(75)).await;

    // 等待超过首档退避后，验证 timer 没有在后台发起新请求。
    assert!(error.is_cancelled());
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn eof_before_terminal_does_not_fabricate_a_terminal_event() {
    let app = app_with_transport(Arc::new(EofWithoutTerminalTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();

    assert!(
        std::str::from_utf8(&body)
            .unwrap()
            .contains("response.output_text.delta")
    );
    assert!(
        !std::str::from_utf8(&body)
            .unwrap()
            .contains("response.completed")
    );
    assert!(!std::str::from_utf8(&body).unwrap().contains("[DONE]"));
}

#[tokio::test]
async fn partial_upstream_stream_failures_close_without_a_retry() {
    let transport = Arc::new(PartialStreamFailureTransport {
        attempts: AtomicUsize::new(0),
    });
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 4096).await.is_err());
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_upstream_sse_closes_the_stream_after_output_starts() {
    let app = app_with_transport(Arc::new(InvalidSseTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 4096).await.is_err());
}

#[tokio::test]
async fn streaming_requests_preserve_non_sse_error_bodies() {
    let app = app_with_transport(Arc::new(NonSseErrorTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        b"\xff".as_slice()
    );
}

#[tokio::test]
async fn streaming_rate_limits_retry_before_output_and_preserve_retry_headers() {
    let transport = Arc::new(RateLimitedTransport::default());
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(transport.attempts.lock().unwrap().to_owned(), 2);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()["retry-after"], "2");
    assert_eq!(response.headers()["x-should-retry"], "true");
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        r#"{"error":{"message":"rate limited"}}"#
    );
}

#[tokio::test]
async fn upstream_timeouts_return_a_safe_gateway_timeout() {
    let app = app_with_transport(Arc::new(TimeoutTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "upstream_timeout");
    assert!(!std::str::from_utf8(&body).unwrap().contains("reqwest"));
}

#[tokio::test]
async fn chat_and_responses_are_forwarded_natively_with_safe_response_headers() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    let cases = [
        (
            "/v1/chat/completions",
            r#"{"model":"public-model","messages":[]}"#,
            "application/json",
            b"{\"id\":\"chat-result\"}".as_slice(),
        ),
        (
            "/v1/responses",
            r#"{"model":"public-model","input":"hello","stream":true}"#,
            "text/event-stream",
            b"event: response.output_text.delta\ndata: {\"delta\":\"hi\"}\n\n".as_slice(),
        ),
    ];

    for (path, request_body, expected_content_type, expected_body) in cases {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(request_body))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], expected_content_type);
        assert_eq!(response.headers()["openai-request-id"], "upstream-id");
        assert!(response.headers().contains_key("x-request-id"));
        assert!(!response.headers().contains_key(SET_COOKIE));
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            expected_body
        );
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(requests[1].path, "/v1/responses");
    for request in requests.iter() {
        assert_eq!(request.authorization, "Bearer upstream-token");
        assert_eq!(request.body["model"], "upstream-model");
    }
}

#[tokio::test]
async fn router_sends_the_candidate_mapped_reasoning_level_upstream() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::XHigh];
    definition.upstream_targets[0].upstream_apis[1]
        .model_rules
        .reasoning_level_mappings = vec![ReasoningLevelMapping {
        downstream: ReasoningLevel::XHigh,
        upstream: "max".to_owned(),
    }];
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-0000000000000000",
        )
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true,"reasoning":{"effort":"xhigh"}}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["reasoning"]["effort"], "max");
}
