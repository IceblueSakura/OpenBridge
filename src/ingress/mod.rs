mod auth;

pub use auth::StaticBearerCredential;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
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
    provider::{CredentialSource, ProviderAdapter, RequestAdapter},
    transport::upstream::{UpstreamClient, UpstreamError, UpstreamTransport},
};

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

async fn forward_native(state: AppState, protocol: Protocol, body: Bytes) -> Response {
    let snapshot = state.config.snapshot();
    let prepared = match prepare_native_request(&snapshot, protocol, body) {
        Ok(prepared) => prepared,
        Err(error) => return route_error(error),
    };
    let Some(deployment) = snapshot.deployment(prepared.deployment_id()) else {
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
    let request = match adapter.encode_request(prepared.request()) {
        Ok(request) => request,
        Err(_) => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "unsupported_request",
                "Request is not supported by the selected provider",
            );
        }
    };
    let upstream = match state.upstream.send(deployment, request, headers).await {
        Ok(response) => response,
        Err(error) => return upstream_error(error),
    };

    let status = upstream.status();
    let response_headers = filtered_upstream_headers(upstream.headers());
    let mut response = Response::builder()
        .status(status)
        .body(upstream.into_body())
        .expect("valid upstream response status");
    response.headers_mut().extend(response_headers);
    response
}

fn filtered_upstream_headers(upstream: &HeaderMap) -> HeaderMap {
    let mut filtered = HeaderMap::new();
    for (name, value) in upstream {
        let name_text = name.as_str();
        if name == CONTENT_TYPE
            || name == RETRY_AFTER
            || name_text == "openai-request-id"
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
