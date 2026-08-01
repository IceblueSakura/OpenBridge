//! Axum Router、中间件与下游认证装配。

use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use http::{
    HeaderName, StatusCode,
    header::{AUTHORIZATION, WWW_AUTHENTICATE},
};
use tower::ServiceBuilder;
use tower_http::{
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    trace::TraceLayer,
};

use crate::{
    credential::CredentialStore,
    identity::UserRegistry,
    observability::{GatewayMetrics, RequestObservation},
};

use super::{
    auth,
    handlers::{chat_completions, health, models, responses},
    lifecycle::{RequestLifecycleGuard, observe_response_body},
    response::api_error,
    state::GatewayState,
};

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
