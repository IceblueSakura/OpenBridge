//! OpenAI-compatible HTTP ingress 与 Native/Bridged Route 编排。
//!
//! 此模块只接受配置允许的 public model，并在进入上游前完成下游 Bearer 认证、body
//! 上限、capability routing 和 provider adapter 选择。它不会接受客户端给出的上游 URL、
//! 认证头或 provider 规则；私有 `AttemptManager` 约束提交下游 response 前的有限退避与
//! fallback，任务取消会销毁尚未完成的上游工作。Native stream 保持字节透明；Bridged
//! stream 使用单请求 renderer 转换完整 SSE event，两者都遵守 framing/terminal 边界。

mod attempt;
mod auth;
mod health;

use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    Extension, Json, Router,
    body::to_bytes,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use http::{
    HeaderMap, HeaderName, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER, WWW_AUTHENTICATE},
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde::Serialize;
use tower::ServiceBuilder;
use tower_http::{
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    trace::TraceLayer,
};

use crate::{
    bridge::{BridgePlan, BridgeStreamRenderer},
    core::ApiProtocol,
    credential::CredentialStore,
    identity::UserRegistry,
    observability::{GatewayMetrics, RequestObservation, UsageCapture},
    pipeline::{RequestPlanningError, analyze_request, plan_request},
    provider::{ProviderAdapter, StreamEventStatus},
    registry::RuntimeRegistry,
    transport::{
        sse::SseDecoder,
        upstream::{TransportError, UpstreamTransport},
    },
};

use self::{
    attempt::{AttemptManager, AttemptStep},
    health::TargetHealth,
};

/// handler 依赖的不可变服务句柄。
///
/// 编译期注册表在启动后保持不可变；上游 transport 与 credential source 以 trait/值对象
/// 注入，因此 contract test 可以验证 HTTP/SSE 边界而无需真实 provider 或明文环境 secret。
#[derive(Clone)]
pub struct GatewayState {
    registry: Arc<RuntimeRegistry>,
    upstream: Arc<dyn UpstreamTransport>,
    users: Arc<UserRegistry>,
    credentials: Arc<CredentialStore>,
    health: Arc<TargetHealth>,
    metrics: GatewayMetrics,
}

impl GatewayState {
    /// 创建可注入 transport 与 credential source 的服务状态。
    pub fn new(
        registry: Arc<RuntimeRegistry>,
        upstream: Arc<dyn UpstreamTransport>,
        users: Arc<UserRegistry>,
        credentials: Arc<CredentialStore>,
    ) -> Self {
        Self {
            registry,
            upstream,
            users,
            credentials,
            health: Arc::new(TargetHealth::default()),
            metrics: GatewayMetrics::default(),
        }
    }

    /// 返回共享的进程内低基数累计值句柄，供 exporter 或测试读取快照。
    pub fn metrics(&self) -> GatewayMetrics {
        self.metrics.clone()
    }
}

#[derive(Clone)]
struct DownstreamAuthState {
    users: Arc<UserRegistry>,
    credentials: Arc<CredentialStore>,
    metrics: GatewayMetrics,
    max_json_body_bytes: usize,
    max_sse_event_bytes: usize,
}

/// 构造公开 health endpoint 与受静态 Bearer 保护的 OpenAI-compatible API。
///
/// body limit 和 request id 在认证前统一施加；`Authorization` 被标为 sensitive，避免
/// `TraceLayer` 或下游日志意外记录 token。`/v1/models` 与业务 endpoint 共用认证层，
/// 从而不暴露内部 Public Model/Route 信息给匿名请求。
pub fn build_router(state: GatewayState) -> Router {
    let max_request_body_bytes = state.registry.limits().max_request_body_bytes();
    let request_id = HeaderName::from_static("x-request-id");
    let middleware = ServiceBuilder::new()
        .layer(SetSensitiveRequestHeadersLayer::new(std::iter::once(
            AUTHORIZATION,
        )))
        .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(request_id))
        .layer(RequestBodyLimitLayer::new(max_request_body_bytes));
    let protected = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/models", get(models))
        .route_layer(middleware::from_fn_with_state(
            DownstreamAuthState {
                users: state.users.clone(),
                credentials: state.credentials.clone(),
                metrics: state.metrics.clone(),
                max_json_body_bytes: state.registry.limits().max_request_body_bytes(),
                max_sse_event_bytes: state.registry.limits().max_sse_event_bytes(),
            },
            require_user,
        ));

    Router::new()
        .route("/healthz", get(health))
        .merge(protected)
        .layer(middleware)
        .with_state(state)
}

async fn require_user(
    State(auth): State<DownstreamAuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    // 提取请求标识和安全审计字段，避免日志依赖未认证的用户对象。
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    // 解析 Bearer token 并执行 constant-time 用户认证。
    let user = auth::bearer_token(request.headers())
        .and_then(|token| auth.users.authenticate(&auth.credentials, token));
    let Some(user) = user else {
        tracing::warn!(%request_id, %method, %path, status = 401, "downstream authentication failed");
        let mut response = api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key",
            "Invalid authentication credentials",
        );
        response
            .headers_mut()
            .insert(WWW_AUTHENTICATE, http::HeaderValue::from_static("Bearer"));
        return response;
    };

    // 将已认证用户绑定到 request span，再交给受保护 handler。
    let span = tracing::info_span!(
        "downstream_request",
        %request_id,
        user_id = %user.id(),
        %method,
        %path,
        protocol = tracing::field::Empty,
        public_model = tracing::field::Empty,
        status = tracing::field::Empty,
    );
    let observation = RequestObservation::new(auth.metrics.clone(), span.clone());
    let mut lifecycle = RequestLifecycleGuard::new(observation.clone());
    request.extensions_mut().insert(user);
    request.extensions_mut().insert(observation.clone());
    let mut response = tracing::Instrument::instrument(next.run(request), span).await;
    observation.record_response_ready(response.status());
    observe_response_body(
        &mut response,
        observation,
        auth.max_json_body_bytes,
        auth.max_sse_event_bytes,
    );
    lifecycle.handoff_to_body();
    response
}

/// 在 response body 建立前捕获 middleware future 被取消的请求。
struct RequestLifecycleGuard {
    observation: Option<RequestObservation>,
}

impl RequestLifecycleGuard {
    /// 创建仍由 request future 负责的生命周期 guard。
    fn new(observation: RequestObservation) -> Self {
        Self {
            observation: Some(observation),
        }
    }

    /// response body wrapper 建立后移交取消和终态责任。
    fn handoff_to_body(&mut self) {
        self.observation.take();
    }
}

impl Drop for RequestLifecycleGuard {
    fn drop(&mut self) {
        // pending send、backoff 或 handler 阶段被取消时尚无 body wrapper，必须在这里收口。
        if let Some(observation) = self.observation.take() {
            observation.cancel();
        }
    }
}

/// 用不改写字节的外层 stream 在真实 EOF、错误或 drop 时结束请求观测。
fn observe_response_body(
    response: &mut Response,
    observation: RequestObservation,
    max_json_body_bytes: usize,
    max_sse_event_bytes: usize,
) {
    // 只为成功 JSON/SSE response 创建有界 usage 解析器。
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let usage = if response.status().is_success() {
        UsageCapture::for_response(content_type, max_json_body_bytes, max_sse_event_bytes)
    } else {
        UsageCapture::None
    };
    let body = std::mem::replace(response.body_mut(), axum::body::Body::empty());
    *response.body_mut() =
        axum::body::Body::new(RequestBodyObserver::new(body, observation, usage));
}

/// 保留原始 HTTP frame，并在 body 的实际消费边界提交请求终态。
struct RequestBodyObserver {
    body: axum::body::Body,
    observation: RequestObservation,
    usage: UsageCapture,
    finished: bool,
}

impl RequestBodyObserver {
    /// 创建尚未产生首字节或终态的透明 body wrapper。
    fn new(body: axum::body::Body, observation: RequestObservation, usage: UsageCapture) -> Self {
        Self {
            body,
            observation,
            usage,
            finished: false,
        }
    }

    fn complete(&mut self) {
        // 正常 EOF 先提交最后一个 usage event，再提交请求终态。
        if self.finished {
            return;
        }
        self.usage.finish(&self.observation);
        self.observation.finish();
        self.finished = true;
    }

    fn fail(&mut self, kind: &'static str) {
        // body error 已是最终可见边界，不能等待下一次 poll 才记录。
        if self.finished {
            return;
        }
        self.observation.record_stream_failure(kind);
        self.observation.finish();
        self.finished = true;
    }
}

impl HttpBody for RequestBodyObserver {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let observer = self.get_mut();
        // 保留所有 data/trailer frame，只在 data frame 上观察首字节和 usage。
        match Pin::new(&mut observer.body).poll_frame(context) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref() {
                    if !chunk.is_empty() {
                        observer.observation.record_first_body_byte();
                    }
                    observer.usage.observe_chunk(&observer.observation, chunk);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                observer.fail("body_error");
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                observer.complete();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

impl Drop for RequestBodyObserver {
    fn drop(&mut self) {
        // 未到 EOF 且未产生 body error 表示下游不再消费 response。
        if !self.finished {
            self.observation.cancel();
        }
    }
}

async fn models(State(state): State<GatewayState>) -> Json<ModelListResponse> {
    let data = state
        .registry
        .public_models()
        .map(|id| PublicModel {
            id: id.to_owned(),
            object: "model",
            owned_by: "openbridge",
        })
        .collect();
    Json(ModelListResponse {
        object: "list",
        data,
    })
}

#[derive(Serialize)]
struct ModelListResponse {
    object: &'static str,
    data: Vec<PublicModel>,
}

#[derive(Serialize)]
struct PublicModel {
    id: String,
    object: &'static str,
    owned_by: &'static str,
}

async fn chat_completions(
    State(state): State<GatewayState>,
    Extension(observation): Extension<RequestObservation>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 先校验原生 JSON media type，再进入统一 route/egress pipeline。
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_request(
        state,
        observation,
        ApiProtocol::ChatCompletions,
        headers,
        body,
    )
    .await
}

async fn responses(
    State(state): State<GatewayState>,
    Extension(observation): Extension<RequestObservation>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 先校验原生 JSON media type，再进入统一 route/egress pipeline。
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_request(state, observation, ApiProtocol::Responses, headers, body).await
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

fn unsupported_media_type() -> Response {
    api_error(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "invalid_content_type",
        "Content-Type must be application/json",
    )
}

/// 将一个已经过 HTTP 输入检查的请求送往有序 Native/Bridged candidate。
///
/// 每次调用共享启动时构建的不可变 registry。仅 streaming 请求可在**尚未返回任何下游
/// body**时重试：一旦 `UpstreamResponse`
/// 被交给客户端，后续 SSE bytes 只能原样继续或以 body error 终止，绝不能拼接第二个
/// 上游尝试。`previous_response_id` 等 target-bound state 会令 pipeline 关闭跨 candidate
/// fallback，但仍可在同一 candidate 上执行有限 pre-output retry。
async fn forward_request(
    state: GatewayState,
    observation: RequestObservation,
    protocol: ApiProtocol,
    downstream_headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 分析请求事实并生成带 capability/fallback 边界的 route plan。
    let registry = state.registry.clone();
    let profile = match analyze_request(protocol, &body) {
        Ok(profile) => profile,
        Err(error) => return route_error(error),
    };
    observation.record_request(protocol, profile.public_model());
    let plan = match plan_request(&registry, &profile, body) {
        Ok(plan) => plan,
        Err(error) => return route_error(error),
    };
    let candidate_count = if plan.allows_fallback() {
        plan.candidates().len()
    } else {
        1
    };
    let mut attempts = AttemptManager::new();
    let observe_cross_request_health = plan.allows_fallback();
    let mut cooldown_skipped = false;

    // 按优先级准备每个 candidate 的 target、credential、adapter 和原生请求。
    'candidates: for (candidate_index, candidate) in
        plan.candidates().iter().take(candidate_count).enumerate()
    {
        attempts.begin_candidate();
        let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured upstream target is unavailable",
            );
        };
        // 新无状态请求跳过仍在 cooldown 的 scope；target-bound continuation 始终尝试原目标。
        if observe_cross_request_health
            && !state.health.is_available(
                candidate.upstream_target_id(),
                target,
                std::time::Instant::now(),
            )
        {
            cooldown_skipped = true;
            observation.record_cooldown_skip(candidate.upstream_target_id());
            continue;
        }
        let Some(upstream_api) = target.upstream_api(candidate.upstream_api_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured native upstream API is unavailable",
            );
        };
        let credential = match state
            .credentials
            .upstream(target.kind(), target.credential().id())
        {
            Ok(credential) => credential,
            Err(_) => {
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "upstream_authentication_error",
                    "Upstream credentials are unavailable",
                );
            }
        };
        let adapter = ProviderAdapter::for_kind(target.kind());
        let headers = match adapter.build_outbound_headers(&credential, &downstream_headers) {
            Ok(headers) => headers,
            Err(_) => {
                return api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "configuration_error",
                    "Provider authentication could not be prepared",
                );
            }
        };
        let request =
            match adapter.prepare_request(candidate.request(), upstream_api.upstream_model()) {
                Ok(request) => request,
                Err(_) => {
                    return api_error(
                        StatusCode::BAD_REQUEST,
                        "unsupported_request",
                        "Request is not supported by the selected provider",
                    );
                }
            };

        // 在尚未向下游提交 response 时执行请求级受限 attempt，并保持 body 的单一来源。
        loop {
            if !attempts.start_attempt() {
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "upstream_attempts_exhausted",
                    "The upstream attempt budget was exhausted",
                );
            }
            observation.record_attempt(
                attempts.attempts_started() as u64,
                candidate.route_id(),
                candidate.upstream_target_id(),
                target.kind(),
                candidate.bridge().is_some(),
            );
            if let Some(mapping) = candidate.reasoning_level_mapping() {
                tracing::info!(
                    downstream_reasoning_level = mapping.downstream.as_wire(),
                    upstream_reasoning_level = mapping.upstream,
                    "reasoning_level_mapped"
                );
            }
            match state
                .upstream
                .send(target, request.clone(), headers.clone())
                .await
            {
                Ok(upstream) if should_retry_status(&adapter, upstream.status()) => {
                    // 在选择 retry/fallback 前记录跨请求 cooldown，但不改变本请求局部 retry 预算。
                    observation.record_attempt_http_result(
                        attempts.attempts_started() as u64,
                        upstream.status(),
                    );
                    let classification = adapter.classify_status(upstream.status());
                    state.health.record_http_failure(
                        candidate.upstream_target_id(),
                        target,
                        classification.kind(),
                        upstream.headers(),
                        std::time::Instant::now(),
                    );
                    let untried_candidates = candidate_count - candidate_index - 1;
                    match attempts.next_step(untried_candidates) {
                        AttemptStep::RetryCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_retry();
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_fallback();
                            continue 'candidates;
                        }
                        AttemptStep::Finish => {
                            return upstream_response(
                                upstream,
                                UpstreamResponseContext {
                                    validate_sse: plan.is_streaming(),
                                    protocol,
                                    adapter,
                                    max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                                    max_json_body_bytes: registry.limits().max_request_body_bytes(),
                                    bridge: candidate.bridge().cloned(),
                                    observation: observation.clone(),
                                },
                            )
                            .await;
                        }
                    }
                }
                Ok(upstream) => {
                    // 只有成功 HTTP response 才清除该 target 的已知 cooldown。
                    observation.record_attempt_http_result(
                        attempts.attempts_started() as u64,
                        upstream.status(),
                    );
                    if upstream.status().is_success() {
                        state
                            .health
                            .record_success(candidate.upstream_target_id(), target);
                    }
                    return upstream_response(
                        upstream,
                        UpstreamResponseContext {
                            validate_sse: plan.is_streaming(),
                            protocol,
                            adapter,
                            max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                            max_json_body_bytes: registry.limits().max_request_body_bytes(),
                            bridge: candidate.bridge().cloned(),
                            observation: observation.clone(),
                        },
                    )
                    .await;
                }
                Err(error) if should_retry_error(&error) => {
                    // timeout/transport failure 只隔离 fault domain，不污染 quota scope。
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_error_kind(&error),
                    );
                    state.health.record_transport_failure(
                        candidate.upstream_target_id(),
                        target,
                        std::time::Instant::now(),
                    );
                    let untried_candidates = candidate_count - candidate_index - 1;
                    match attempts.next_step(untried_candidates) {
                        AttemptStep::RetryCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_retry();
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_fallback();
                            continue 'candidates;
                        }
                        AttemptStep::Finish => return upstream_error(error),
                    }
                }
                Err(error) => {
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_error_kind(&error),
                    );
                    return upstream_error(error);
                }
            }
        }
    }

    if cooldown_skipped {
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "upstream_cooldown",
            "All compatible upstream targets are temporarily unavailable",
        )
    } else {
        api_error(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "The upstream request failed",
        )
    }
}

fn should_retry_status(adapter: &ProviderAdapter, status: StatusCode) -> bool {
    adapter.classify_status(status).retry_hint() == crate::provider::RetryHint::BeforeFirstEvent
}

fn should_retry_error(error: &TransportError) -> bool {
    matches!(error, TransportError::Timeout | TransportError::Request(_))
}

fn transport_error_kind(error: &TransportError) -> &'static str {
    match error {
        TransportError::ClientBuild(_) => "client_build",
        TransportError::Request(_) => "request",
        TransportError::Timeout => "timeout",
        TransportError::InvalidTarget => "invalid_target",
    }
}

/// 将上游 status、安全响应头和 Native/Bridged body 交给下游。
///
/// SSE 仅在原请求要求 streaming、上游返回成功状态且 `Content-Type` 确为
/// `text/event-stream` 时验证。错误响应即使对应 streaming request 也可能是 JSON 或其他
/// 诊断 body；对其做 SSE 解码会破坏可见的 HTTP 错误语义。
/// 一次已选定候选的响应转换、SSE 和观测上下文。
struct UpstreamResponseContext {
    validate_sse: bool,
    protocol: ApiProtocol,
    adapter: ProviderAdapter,
    max_sse_event_bytes: usize,
    max_json_body_bytes: usize,
    bridge: Option<BridgePlan>,
    observation: RequestObservation,
}

async fn upstream_response(
    upstream: crate::transport::upstream::UpstreamResponse,
    context: UpstreamResponseContext,
) -> Response {
    // 拆分已固定的响应处理事实，避免函数调用点遗漏协议或观测边界。
    let UpstreamResponseContext {
        validate_sse,
        protocol,
        adapter,
        max_sse_event_bytes,
        max_json_body_bytes,
        bridge,
        observation,
    } = context;
    // 提取 status 和安全响应头，并仅对成功 SSE response 启用观察器。
    let status = upstream.status();
    let response_headers = filtered_upstream_headers(upstream.headers());
    let is_sse = upstream
        .headers()
        .get(CONTENT_TYPE)
        .is_some_and(|content_type| {
            content_type
                .to_str()
                .is_ok_and(|value| value.starts_with("text/event-stream"))
        });
    if bridge.is_some() && validate_sse && status.is_success() && !is_sse {
        return api_error(
            StatusCode::BAD_GATEWAY,
            "invalid_upstream_response",
            "The upstream response could not be converted",
        );
    }
    // 保持非 SSE 或错误 body 原样透传，避免破坏上游诊断语义。
    let body = if validate_sse && status.is_success() && is_sse {
        if let Some(bridge) = bridge {
            bridge_sse_body(
                upstream.into_body(),
                bridge.stream_renderer(),
                max_sse_event_bytes,
            )
        } else {
            validate_sse_body(
                upstream.into_body(),
                protocol,
                adapter,
                max_sse_event_bytes,
                observation,
            )
        }
    } else if status.is_success() {
        if let Some(bridge) = bridge {
            let upstream_body = match to_bytes(upstream.into_body(), max_json_body_bytes).await {
                Ok(body) => body,
                Err(_) => {
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    );
                }
            };
            match bridge.render_non_stream(upstream_body) {
                Ok(body) => axum::body::Body::from(body),
                Err(_) => {
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    );
                }
            }
        } else {
            upstream.into_body()
        }
    } else {
        upstream.into_body()
    };
    let mut response = Response::builder()
        .status(status)
        .body(body)
        .expect("valid upstream response status");
    response.headers_mut().extend(response_headers);
    response
}

/// 增量解码上游 SSE，并用单请求 Bridge renderer 生成目标协议 event。
fn bridge_sse_body(
    body: axum::body::Body,
    renderer: BridgeStreamRenderer,
    max_sse_event_bytes: usize,
) -> axum::body::Body {
    // 保持 source、decoder 与 renderer 同生命周期，下游 drop 会同步取消上游 body。
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            renderer,
            false,
        ),
        move |(mut source, mut decoder, mut renderer, finished)| async move {
            if finished {
                return None;
            }
            match source.as_mut().next().await {
                Some(Ok(chunk)) => {
                    let events = match decoder.push(&chunk) {
                        Ok(events) => events,
                        Err(_) => {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, true),
                            ));
                        }
                    };
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, true),
                                ));
                            }
                        }
                    }
                    Some((
                        Ok::<_, io::Error>(Bytes::from(output)),
                        (source, decoder, renderer, false),
                    ))
                }
                Some(Err(_)) => Some((
                    Err(io::Error::other(
                        "upstream SSE stream terminated unexpectedly",
                    )),
                    (source, decoder, renderer, true),
                )),
                None => {
                    let events = match decoder.finish() {
                        Ok(events) => events,
                        Err(_) => {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, true),
                            ));
                        }
                    };
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, true),
                                ));
                            }
                        }
                    }
                    match renderer.finish() {
                        Ok(bytes) => output.extend_from_slice(&bytes),
                        Err(_) => {
                            return Some((
                                Err(io::Error::other("upstream bridge stream is invalid")),
                                (source, decoder, renderer, true),
                            ));
                        }
                    }
                    if output.is_empty() {
                        None
                    } else {
                        Some((
                            Ok::<_, io::Error>(Bytes::from(output)),
                            (source, decoder, renderer, true),
                        ))
                    }
                }
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

/// 在不重写原始 bytes 的前提下观察上游 SSE 生命周期。
///
/// decoder 仅用于处理跨网络 chunk 的 UTF-8/SSE framing，并委托 provider adapter 识别协议
/// terminal event。合法 EOF 但未看到 terminal 会保留已收到的 bytes 并记录 warning；无效
/// framing、无效 UTF-8 或上游 body error 则以 stream error 关闭。body 被下游丢弃时，
/// `source` 一并 drop，从而取消 reqwest 的上游字节流。
fn validate_sse_body(
    body: axum::body::Body,
    protocol: ApiProtocol,
    adapter: ProviderAdapter,
    max_sse_event_bytes: usize,
    observation: RequestObservation,
) -> axum::body::Body {
    // 创建保持上游 source 生命周期的增量 SSE decoder。
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            false,
            false,
            observation,
        ),
        move |(mut source, mut decoder, mut terminal_seen, finished, observation)| async move {
            if finished {
                return None;
            }
            // 读取下一个上游 chunk，并只观察 framing/terminal，不改写原始 bytes。
            match source.as_mut().next().await {
                Some(Ok(chunk)) => match decoder.push(&chunk) {
                    Ok(events) => {
                        match observe_sse_events(
                            adapter,
                            protocol,
                            events,
                            &mut terminal_seen,
                            &observation,
                        ) {
                            Ok(()) => Some((
                                Ok::<_, io::Error>(chunk),
                                (source, decoder, terminal_seen, false, observation),
                            )),
                            Err(()) => Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            )),
                        }
                    }
                    Err(_) => Some((
                        Err(io::Error::other("upstream SSE stream is invalid")),
                        (source, decoder, terminal_seen, true, observation),
                    )),
                },
                Some(Err(_)) => Some((
                    Err(io::Error::other(
                        "upstream SSE stream terminated unexpectedly",
                    )),
                    (source, decoder, terminal_seen, true, observation),
                )),
                None => match decoder.finish() {
                    Ok(events) => {
                        if observe_sse_events(
                            adapter,
                            protocol,
                            events,
                            &mut terminal_seen,
                            &observation,
                        )
                        .is_err()
                        {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            ));
                        }
                        if !terminal_seen {
                            observation.record_stream_failure("sse_eof_before_terminal");
                            tracing::warn!(
                                ?protocol,
                                "upstream SSE stream ended before a terminal event"
                            );
                        }
                        None
                    }
                    Err(_) => Some((
                        Err(io::Error::other("upstream SSE stream is invalid")),
                        (source, decoder, terminal_seen, true, observation),
                    )),
                },
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

fn observe_sse_events(
    adapter: ProviderAdapter,
    protocol: ApiProtocol,
    events: Vec<crate::transport::sse::SseEvent>,
    terminal_seen: &mut bool,
    observation: &RequestObservation,
) -> Result<(), ()> {
    // 委托 provider adapter 分类 event，并记录是否已看到 terminal。
    for event in events {
        let decoded = adapter
            .classify_sse_event(protocol, event)
            .map_err(|_| ())?;
        match decoded.status() {
            StreamEventStatus::Continue => {}
            StreamEventStatus::Completed => *terminal_seen = true,
            StreamEventStatus::Failed => {
                *terminal_seen = true;
                observation.record_stream_failure("provider_terminal_failed");
            }
        }
    }
    Ok(())
}

/// 仅透传 OpenAI-compatible client 需要且不会改变 proxy 安全边界的上游响应头。
///
/// 不透传 cookie、认证、连接管理或任意自定义 header；这样上游无法借 proxy 向客户端设置
/// 会话状态，也不会泄露内部 transport 细节。
fn filtered_upstream_headers(upstream: &HeaderMap) -> HeaderMap {
    // 仅复制协议所需且不会泄露认证/连接状态的响应头。
    let mut filtered = HeaderMap::new();
    for (name, value) in upstream {
        let name_text = name.as_str();
        if name == CONTENT_TYPE
            || name == RETRY_AFTER
            || name_text == "openai-request-id"
            || name_text == "x-should-retry"
            || name_text.starts_with("x-ratelimit-")
        {
            filtered.append(name.clone(), value.clone());
        }
    }
    filtered
}

fn route_error(error: RequestPlanningError) -> Response {
    match error {
        RequestPlanningError::InvalidJson | RequestPlanningError::MissingModel => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "Request body is invalid",
        ),
        RequestPlanningError::UnknownModel | RequestPlanningError::NoRoute => api_error(
            StatusCode::NOT_FOUND,
            "model_not_found",
            "The requested model is not available",
        ),
        RequestPlanningError::UnsupportedProtocol
        | RequestPlanningError::StreamingUnsupported
        | RequestPlanningError::UnsupportedCapabilities
        | RequestPlanningError::OutputLimitExceeded
        | RequestPlanningError::ReasoningUnsupported
        | RequestPlanningError::ReasoningLevelUnsupported => api_error(
            StatusCode::BAD_REQUEST,
            "unsupported_request",
            "The selected model does not support this request",
        ),
    }
}

fn upstream_error(error: TransportError) -> Response {
    match error {
        TransportError::Timeout => api_error(
            StatusCode::GATEWAY_TIMEOUT,
            "upstream_timeout",
            "The upstream request timed out",
        ),
        _ => api_error(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "The upstream request failed",
        ),
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    registry_version: String,
}

async fn health(State(state): State<GatewayState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        registry_version: state.registry.version().as_str().to_owned(),
    })
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    message: &'static str,
    r#type: &'static str,
    param: Option<&'static str>,
    code: &'static str,
}

fn api_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            error: ErrorBody {
                message,
                r#type: "invalid_request_error",
                param: None,
                code,
            },
        }),
    )
        .into_response()
}
