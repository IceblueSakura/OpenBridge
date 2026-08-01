//! OpenAI-compatible HTTP ingress 与原生转发编排。
//!
//! 此模块只接受配置允许的 public model，并在进入上游前完成下游 Bearer 认证、body
//! 上限、capability routing 和 provider adapter 选择。它不会接受客户端给出的上游 URL、
//! 认证头或 provider 规则；streaming response 保持字节透明，同时用 SSE decoder 只作
//! framing/terminal 校验，不重新渲染业务 event。

mod auth;

pub use auth::DownstreamCredential;

use std::{io, sync::Arc};

use axum::{
    Json, Router,
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
use serde::Serialize;
use tower::ServiceBuilder;
use tower_http::{
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    trace::TraceLayer,
};

use crate::{
    core::ApiProtocol,
    pipeline::{RequestPlanningError, analyze_request, plan_request},
    provider::{CredentialSource, ProviderAdapter, StreamEventStatus},
    registry::RuntimeRegistry,
    transport::{
        sse::SseDecoder,
        upstream::{TransportError, UpstreamClient, UpstreamTransport},
    },
};

/// handler 依赖的不可变服务句柄。
///
/// 编译期注册表在启动后保持不可变；上游 transport 与 credential source 以 trait/值对象
/// 注入，因此 contract test 可以验证 HTTP/SSE 边界而无需真实 provider 或明文环境 secret。
#[derive(Clone)]
pub struct GatewayState {
    registry: Arc<RuntimeRegistry>,
    upstream: Arc<dyn UpstreamTransport>,
    downstream_credential: Arc<DownstreamCredential>,
    upstream_credentials: Arc<CredentialSource>,
}

impl GatewayState {
    /// 创建可注入 transport 与 credential source 的服务状态。
    pub fn new(
        registry: Arc<RuntimeRegistry>,
        upstream: Arc<dyn UpstreamTransport>,
        downstream_credential: DownstreamCredential,
        upstream_credentials: CredentialSource,
    ) -> Self {
        Self {
            registry,
            upstream,
            downstream_credential: Arc::new(downstream_credential),
            upstream_credentials: Arc::new(upstream_credentials),
        }
    }

    /// 创建使用环境变量读取上游 credential 的生产运行时状态。
    pub fn with_environment_credentials(
        registry: Arc<RuntimeRegistry>,
        upstream: UpstreamClient,
        downstream_credential: DownstreamCredential,
    ) -> Self {
        Self::new(
            registry,
            Arc::new(upstream),
            downstream_credential,
            CredentialSource::environment(),
        )
    }
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
            state.downstream_credential.clone(),
            require_downstream_credential,
        ));

    Router::new()
        .route("/healthz", get(health))
        .merge(protected)
        .layer(middleware)
        .with_state(state)
}

async fn require_downstream_credential(
    State(credential): State<Arc<DownstreamCredential>>,
    request: Request,
    next: Next,
) -> Response {
    if credential.authenticate(request.headers()) {
        next.run(request).await
    } else {
        let mut response = api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key",
            "Invalid authentication credentials",
        );
        response
            .headers_mut()
            .insert(WWW_AUTHENTICATE, http::HeaderValue::from_static("Bearer"));
        response
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
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_native(state, ApiProtocol::ChatCompletions, body).await
}

async fn responses(State(state): State<GatewayState>, headers: HeaderMap, body: Bytes) -> Response {
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_native(state, ApiProtocol::Responses, body).await
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

/// 将一个已经过 HTTP 输入检查的原生请求送往有序 candidate。
///
/// 每次调用共享启动时构建的不可变 registry。仅 streaming 请求可在**尚未返回任何下游
/// body**时重试：一旦 `UpstreamResponse`
/// 被交给客户端，后续 SSE bytes 只能原样继续或以 body error 终止，绝不能拼接第二个
/// 上游尝试。`previous_response_id` 等 target-bound state 会令 pipeline 关闭跨 candidate
/// fallback，但仍可在同一 candidate 上执行有限 pre-output retry。
async fn forward_native(state: GatewayState, protocol: ApiProtocol, body: Bytes) -> Response {
    const MAX_UPSTREAM_ATTEMPTS: usize = 2;

    let registry = state.registry.clone();
    let profile = match analyze_request(protocol, &body) {
        Ok(profile) => profile,
        Err(error) => return route_error(error),
    };
    let plan = match plan_request(&registry, &profile, body) {
        Ok(plan) => plan,
        Err(error) => return route_error(error),
    };
    let candidate_count = if plan.allows_fallback() {
        plan.candidates().len()
    } else {
        1
    };

    'candidates: for (candidate_index, candidate) in
        plan.candidates().iter().take(candidate_count).enumerate()
    {
        let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured upstream target is unavailable",
            );
        };
        let Some(upstream_api) = target.upstream_api(candidate.upstream_api_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured native upstream API is unavailable",
            );
        };
        let credential = match state.upstream_credentials.resolve(
            target.kind(),
            target.credential().id(),
            target.credential().secret_reference().locator(),
        ) {
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
        let headers = match adapter.build_outbound_headers(&credential) {
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

        for attempt in 0..MAX_UPSTREAM_ATTEMPTS {
            match state
                .upstream
                .send(target, request.clone(), headers.clone())
                .await
            {
                Ok(upstream)
                    if should_retry_status(&adapter, upstream.status()) && plan.is_streaming() =>
                {
                    if attempt + 1 < MAX_UPSTREAM_ATTEMPTS {
                        continue;
                    }
                    if candidate_index + 1 < candidate_count {
                        continue 'candidates;
                    }
                    return upstream_response(
                        upstream,
                        plan.is_streaming(),
                        protocol,
                        adapter,
                        registry.limits().max_sse_event_bytes(),
                    );
                }
                Ok(upstream) => {
                    return upstream_response(
                        upstream,
                        plan.is_streaming(),
                        protocol,
                        adapter,
                        registry.limits().max_sse_event_bytes(),
                    );
                }
                Err(error) if should_retry_error(&error) && plan.is_streaming() => {
                    if attempt + 1 < MAX_UPSTREAM_ATTEMPTS {
                        continue;
                    }
                    if candidate_index + 1 < candidate_count {
                        continue 'candidates;
                    }
                    return upstream_error(error);
                }
                Err(error) => return upstream_error(error),
            }
        }
    }

    api_error(
        StatusCode::BAD_GATEWAY,
        "upstream_error",
        "The upstream request failed",
    )
}

fn should_retry_status(adapter: &ProviderAdapter, status: StatusCode) -> bool {
    adapter.classify_status(status).retry_hint() == crate::provider::RetryHint::BeforeFirstEvent
}

fn should_retry_error(error: &TransportError) -> bool {
    matches!(error, TransportError::Timeout | TransportError::Request(_))
}

/// 将上游 status、白名单响应头和 body 交给下游。
///
/// SSE 仅在原请求要求 streaming、上游返回成功状态且 `Content-Type` 确为
/// `text/event-stream` 时验证。错误响应即使对应 streaming request 也可能是 JSON 或其他
/// 诊断 body；对其做 SSE 解码会破坏可见的 HTTP 错误语义。
fn upstream_response(
    upstream: crate::transport::upstream::UpstreamResponse,
    validate_sse: bool,
    protocol: ApiProtocol,
    adapter: ProviderAdapter,
    max_sse_event_bytes: usize,
) -> Response {
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
    let body = if validate_sse && status.is_success() && is_sse {
        validate_sse_body(upstream.into_body(), protocol, adapter, max_sse_event_bytes)
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
) -> axum::body::Body {
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            false,
            false,
        ),
        move |(mut source, mut decoder, mut terminal_seen, finished)| async move {
            if finished {
                return None;
            }
            match source.as_mut().next().await {
                Some(Ok(chunk)) => match decoder.push(&chunk) {
                    Ok(events) => {
                        match observe_sse_events(adapter, protocol, events, &mut terminal_seen) {
                            Ok(()) => Some((
                                Ok::<_, io::Error>(chunk),
                                (source, decoder, terminal_seen, false),
                            )),
                            Err(()) => Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true),
                            )),
                        }
                    }
                    Err(_) => Some((
                        Err(io::Error::other("upstream SSE stream is invalid")),
                        (source, decoder, terminal_seen, true),
                    )),
                },
                Some(Err(_)) => Some((
                    Err(io::Error::other(
                        "upstream SSE stream terminated unexpectedly",
                    )),
                    (source, decoder, terminal_seen, true),
                )),
                None => match decoder.finish() {
                    Ok(events) => {
                        if observe_sse_events(adapter, protocol, events, &mut terminal_seen)
                            .is_err()
                        {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true),
                            ));
                        }
                        if !terminal_seen {
                            tracing::warn!(
                                ?protocol,
                                "upstream SSE stream ended before a terminal event"
                            );
                        }
                        None
                    }
                    Err(_) => Some((
                        Err(io::Error::other("upstream SSE stream is invalid")),
                        (source, decoder, terminal_seen, true),
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
) -> Result<(), ()> {
    for event in events {
        let decoded = adapter
            .classify_sse_event(protocol, event)
            .map_err(|_| ())?;
        if decoded.status() != StreamEventStatus::Continue {
            *terminal_seen = true;
        }
    }
    Ok(())
}

/// 仅透传 OpenAI-compatible client 需要且不会改变 proxy 安全边界的上游响应头。
///
/// 不透传 cookie、认证、连接管理或任意自定义 header；这样上游无法借 proxy 向客户端设置
/// 会话状态，也不会泄露内部 transport 细节。
fn filtered_upstream_headers(upstream: &HeaderMap) -> HeaderMap {
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
