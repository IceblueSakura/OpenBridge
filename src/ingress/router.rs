//! Axum Router, middleware, and downstream authentication assembly.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use http::{
    HeaderName, Method, StatusCode,
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
    config::HttpLoggingConfig,
    credential::CredentialStore,
    identity::UserRegistry,
    mcp,
    observability::{GatewayMetrics, HttpJsonlWriter, RequestObservation},
};

use super::{
    auth,
    handlers::{
        chat_completions, embeddings, extended_model, extended_models, health, model, models,
        responses,
    },
    lifecycle::{RequestLifecycleGuard, observe_request_body, observe_response_body},
    openapi::{openapi_spec, swagger_ui},
    response::{api_error, embedding_request_too_large},
    state::GatewayState,
};

#[derive(Clone)]
struct DownstreamAuthState {
    users: Arc<UserRegistry>,
    credentials: Arc<CredentialStore>,
    metrics: GatewayMetrics,
    http_logging: HttpLoggingConfig,
    jsonl_writer: Option<HttpJsonlWriter>,
    max_request_body_bytes: usize,
    max_response_body_bytes: usize,
    max_sse_event_bytes: usize,
}

/// Builds public resources plus OpenAI-compatible and MCP APIs protected by static Bearer authentication.
///
/// Body limits and request IDs are applied before authentication; `Authorization` is marked
/// sensitive so `TraceLayer` and downstream logs cannot accidentally record the token.
/// `/v1/models` shares the authentication layer with business endpoints and therefore does not
/// expose internal Public Model/Route information to anonymous requests. `/mcp` adds an
/// origin-rejection layer outside the same authentication boundary.
pub fn build_router(state: GatewayState) -> Router {
    // Prepare global middleware for request IDs, sensitive headers, tracing, and body-size protection.
    let max_request_body_bytes = state.registry.limits().max_request_body_bytes();
    let request_id = HeaderName::from_static("x-request-id");
    let middleware = ServiceBuilder::new()
        .layer(SetSensitiveRequestHeadersLayer::new(std::iter::once(
            AUTHORIZATION,
        )))
        .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(request_id))
        .layer(middleware::from_fn(normalize_embedding_request_limit))
        .layer(RequestBodyLimitLayer::new(max_request_body_bytes));
    let downstream_auth = DownstreamAuthState {
        users: state.users.clone(),
        credentials: state.credentials.clone(),
        metrics: state.metrics.clone(),
        http_logging: state.registry.http_logging().clone(),
        jsonl_writer: state.http_jsonl_writer.clone(),
        max_request_body_bytes,
        max_response_body_bytes: state.registry.limits().max_json_response_body_bytes(),
        max_sse_event_bytes: state.registry.limits().max_sse_event_bytes(),
    };

    // Assemble business and model-list endpoints with shared Bearer authentication.
    let protected = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/models", get(models))
        .route("/v1/models/{model}", get(model))
        .route("/openbridge/v1/models", get(extended_models))
        .route("/openbridge/v1/models/{model}", get(extended_model))
        .route_layer(middleware::from_fn_with_state(
            downstream_auth.clone(),
            require_user,
        ));

    // Expose the originless MCP service behind the same static downstream identity boundary.
    let mcp = Router::new()
        .nest_service("/mcp", mcp::service())
        .route_layer(middleware::from_fn_with_state(
            downstream_auth,
            require_user,
        ))
        .route_layer(middleware::from_fn(mcp::reject_origin));

    // Expose unauthenticated health, OpenAPI, and Swagger UI resources with the shared GatewayState.
    Router::new()
        .route("/healthz", get(health))
        .route("/openapi.yaml", get(openapi_spec))
        .route("/swagger-ui", get(swagger_ui))
        .route("/swagger-ui/", get(swagger_ui))
        .merge(protected)
        .merge(mcp)
        .layer(middleware)
        .with_state(state)
}

/// Replaces only the Embeddings body-limit response with its exact JSON error contract.
async fn normalize_embedding_request_limit(request: Request, next: Next) -> Response {
    // Capture endpoint identity before the body-limit service consumes the request.
    let is_embeddings =
        request.method() == Method::POST && request.uri().path() == "/v1/embeddings";

    // Run the configured hard-limit service and retain all non-limit responses unchanged.
    let response = next.run(request).await;
    if is_embeddings && response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        embedding_request_too_large()
    } else {
        response
    }
}

/// Authenticates a downstream Bearer token and binds non-sensitive user identity and request observation to the handler.
async fn require_user(
    State(auth): State<DownstreamAuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract request identifiers and safe audit fields without relying on an unauthenticated user object.
    let request_id_header = request.headers().get("x-request-id").cloned();
    let request_id = request_id_header
        .as_ref()
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    // Parse the Bearer token and authenticate the user with constant-time comparison.
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

    // Bind the authenticated user before invoking any protected handler.
    request.extensions_mut().insert(user.clone());

    // Bind the observed request to a span and freeze its startup-owned local logging policy.
    let span = tracing::info_span!("downstream_request", %request_id);
    let observation = RequestObservation::new_with_http_logging(
        auth.metrics.clone(),
        span.clone(),
        request_id.clone(),
        auth.http_logging,
        auth.jsonl_writer.clone(),
    );
    let mut lifecycle = RequestLifecycleGuard::new(observation.clone());

    // Capture only explicitly enabled authenticated request headers and a bounded body copy.
    observation.log_request_headers(&method, &path, request.headers());
    observe_request_body(
        &mut request,
        observation.clone(),
        auth.max_request_body_bytes,
    );
    request.extensions_mut().insert(observation.clone());

    // Run the protected handler under the request span until response headers are ready.
    let mut response = tracing::Instrument::instrument(next.run(request), span).await;
    if let Some(request_id_header) = request_id_header {
        // Make the outer propagation result visible to the response snapshot without changing its final value.
        response
            .headers_mut()
            .insert("x-request-id", request_id_header);
    }
    observation.record_response_ready(response.status());
    observation.log_response_headers(response.status(), response.headers());

    // Transfer terminal and optional bounded response-body logging to the transparent body wrapper.
    observe_response_body(
        &mut response,
        observation,
        auth.max_sse_event_bytes,
        auth.max_response_body_bytes,
    );
    lifecycle.handoff_to_body();
    response
}
