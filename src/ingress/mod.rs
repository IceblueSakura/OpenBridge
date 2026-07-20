//! OpenAI-compatible HTTP ingress 与原生转发编排。
//!
//! 此模块只接受配置允许的 public model，并在进入上游前完成下游 Bearer 认证、body
//! 上限、capability routing 和 provider adapter 选择。它不会接受客户端给出的上游 URL、
//! 认证头或 provider 规则；streaming response 保持字节透明，同时用 SSE decoder 只作
//! framing/terminal 校验，不重新渲染业务 event。

mod auth;

pub use auth::StaticBearerCredential;

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
    config::ConfigManager,
    core::Protocol,
    pipeline::{RouteError, prepare_native_request},
    provider::{
        CredentialSource, ErrorAdapter, EventDisposition, ProviderAdapter, RequestAdapter,
        ResponseAdapter,
    },
    transport::{
        sse::SseDecoder,
        upstream::{UpstreamClient, UpstreamError, UpstreamTransport},
    },
};

/// handler 依赖的不可变服务句柄。
///
/// `ConfigManager` 提供 request 开始时固定的 route snapshot；上游 transport 与 credential
/// source 以 trait/值对象注入，因此 contract test 可以验证 HTTP/SSE 边界而无需真实
/// provider 或明文环境 secret。
#[derive(Clone)]
pub struct AppState {
    config: Arc<ConfigManager>,
    upstream: Arc<dyn UpstreamTransport>,
    downstream_credential: Arc<StaticBearerCredential>,
    upstream_credentials: Arc<CredentialSource>,
}

impl AppState {
    pub fn new(
        config: Arc<ConfigManager>,
        upstream: Arc<dyn UpstreamTransport>,
        downstream_credential: StaticBearerCredential,
        upstream_credentials: CredentialSource,
    ) -> Self {
        Self {
            config,
            upstream,
            downstream_credential: Arc::new(downstream_credential),
            upstream_credentials: Arc::new(upstream_credentials),
        }
    }

    pub fn with_environment_credentials(
        config: Arc<ConfigManager>,
        upstream: UpstreamClient,
        downstream_credential: StaticBearerCredential,
    ) -> Self {
        Self::new(
            config,
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
/// 从而不暴露内部 alias/deployment 信息给匿名请求。
pub fn build_router(state: AppState) -> Router {
    let max_request_body_bytes = state.config.snapshot().limits().max_request_body_bytes();
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
    State(credential): State<Arc<StaticBearerCredential>>,
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

async fn models(State(state): State<AppState>) -> Json<ModelListResponse> {
    let data = state
        .config
        .snapshot()
        .public_aliases()
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
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_native(state, Protocol::ChatCompletions, body).await
}

async fn responses(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_native(state, Protocol::Responses, body).await
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
/// 每次调用先获取一个 snapshot，整个循环都使用它，因此 route reload 不会改变正在执行的
/// 请求。仅 streaming 请求可在**尚未返回任何下游 body**时重试：一旦 `UpstreamResponse`
/// 被交给客户端，后续 SSE bytes 只能原样继续或以 body error 终止，绝不能拼接第二个
/// 上游尝试。`previous_response_id` 等 provider-bound state 会令 pipeline 关闭跨 candidate
/// fallback，但仍可在同一 candidate 上执行有限 pre-output retry。
async fn forward_native(state: AppState, protocol: Protocol, body: Bytes) -> Response {
    const MAX_UPSTREAM_ATTEMPTS: usize = 2;

    let snapshot = state.config.snapshot();
    let prepared = match prepare_native_request(&snapshot, protocol, body) {
        Ok(prepared) => prepared,
        Err(error) => return route_error(error),
    };
    let candidate_count = if prepared.allows_fallback() {
        prepared.candidates().len()
    } else {
        1
    };

    'candidates: for (candidate_index, candidate) in prepared
        .candidates()
        .iter()
        .take(candidate_count)
        .enumerate()
    {
        let Some(deployment) = snapshot.deployment(candidate.deployment_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured deployment is unavailable",
            );
        };
        let Some(provider) = snapshot.provider(deployment.provider_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured provider is unavailable",
            );
        };
        let credential = match state.upstream_credentials.resolve(
            provider.kind(),
            provider.credential().id(),
            provider.credential().secret_reference().locator(),
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
        let adapter = ProviderAdapter::for_kind(provider.kind());
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
        let request = match adapter.encode_request(candidate.request()) {
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
                .send(deployment, request.clone(), headers.clone())
                .await
            {
                Ok(upstream)
                    if should_retry_status(&adapter, upstream.status())
                        && prepared.is_streaming() =>
                {
                    if attempt + 1 < MAX_UPSTREAM_ATTEMPTS {
                        continue;
                    }
                    if candidate_index + 1 < candidate_count {
                        continue 'candidates;
                    }
                    return upstream_response(
                        upstream,
                        prepared.is_streaming(),
                        protocol,
                        adapter,
                        snapshot.limits().max_sse_event_bytes(),
                    );
                }
                Ok(upstream) => {
                    return upstream_response(
                        upstream,
                        prepared.is_streaming(),
                        protocol,
                        adapter,
                        snapshot.limits().max_sse_event_bytes(),
                    );
                }
                Err(error) if should_retry_error(&error) && prepared.is_streaming() => {
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

fn should_retry_error(error: &UpstreamError) -> bool {
    matches!(error, UpstreamError::Timeout | UpstreamError::Request(_))
}

/// 将上游 status、白名单响应头和 body 交给下游。
///
/// SSE 仅在原请求要求 streaming、上游返回成功状态且 `Content-Type` 确为
/// `text/event-stream` 时验证。错误响应即使对应 streaming request 也可能是 JSON 或其他
/// 诊断 body；对其做 SSE 解码会破坏可见的 HTTP 错误语义。
fn upstream_response(
    upstream: crate::transport::upstream::UpstreamResponse,
    validate_sse: bool,
    protocol: Protocol,
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
    protocol: Protocol,
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
    protocol: Protocol,
    events: Vec<crate::transport::sse::SseEvent>,
    terminal_seen: &mut bool,
) -> Result<(), ()> {
    for event in events {
        let decoded = adapter.decode_event(protocol, event).map_err(|_| ())?;
        if decoded.disposition() != EventDisposition::Continue {
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

fn route_error(error: RouteError) -> Response {
    match error {
        RouteError::InvalidJson | RouteError::MissingModel => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "Request body is invalid",
        ),
        RouteError::UnknownModel | RouteError::NoDeployment => api_error(
            StatusCode::NOT_FOUND,
            "model_not_found",
            "The requested model is not available",
        ),
        RouteError::UnsupportedProtocol
        | RouteError::StreamingUnsupported
        | RouteError::UnsupportedCapabilities => api_error(
            StatusCode::BAD_REQUEST,
            "unsupported_request",
            "The selected model does not support this request",
        ),
    }
}

fn upstream_error(error: UpstreamError) -> Response {
    match error {
        UpstreamError::Timeout => api_error(
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
    config_version: String,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let snapshot = state.config.snapshot();
    Json(HealthResponse {
        status: "ok",
        config_version: snapshot.version().as_str().to_owned(),
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
