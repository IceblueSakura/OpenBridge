//! Safe downstream response headers and normalized error responses.

use axum::{
    Json,
    response::{IntoResponse, Response},
};
use http::{
    HeaderMap, StatusCode,
    header::{CONTENT_TYPE, RETRY_AFTER},
};
use serde::Serialize;

use crate::{
    pipeline::{EmbeddingRequestError, RequestPlanningError},
    transport::upstream::{TransportError, UpstreamResponse},
};

/// Forwards only upstream response headers required by OpenAI-compatible clients that do not alter the proxy security boundary.
///
/// Cookie, authentication, connection-management, and arbitrary custom headers are not forwarded;
/// upstream services cannot use the proxy to set client session state or reveal transport details.
pub(super) fn filtered_upstream_headers(upstream: &HeaderMap) -> HeaderMap {
    // Copy only protocol-required headers that do not reveal authentication or connection state.
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

/// Maps request-planning errors to stable downstream HTTP errors without exposing Route details.
pub(super) fn route_error(error: RequestPlanningError) -> Response {
    match error {
        RequestPlanningError::InvalidJson
        | RequestPlanningError::MissingModel
        | RequestPlanningError::InvalidMessages
        | RequestPlanningError::InvalidReasoningConfiguration
        | RequestPlanningError::InvalidStreamOptions
        | RequestPlanningError::InvalidMultimodalInput => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "Request body is invalid",
        ),
        RequestPlanningError::InvalidInstructions => typed_api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "invalid_request_error",
            "Instructions must be a non-blank string",
            Some("instructions"),
        ),
        RequestPlanningError::InvalidStore => typed_api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "invalid_request_error",
            "Store must be false when present",
            Some("store"),
        ),
        RequestPlanningError::UnknownParameter(parameter) => typed_api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "unknown_parameter",
            "The request contains an unknown top-level parameter",
            Some(&parameter),
        ),
        RequestPlanningError::UnknownModel | RequestPlanningError::NoRoute => model_not_found(),
        RequestPlanningError::UnimplementedCapabilities => api_error(
            StatusCode::BAD_REQUEST,
            "unimplemented_request",
            "The request uses a capability that is not implemented",
        ),
        RequestPlanningError::UnsupportedProtocol
        | RequestPlanningError::StreamingUnsupported
        | RequestPlanningError::NonStreamingUnsupported
        | RequestPlanningError::UnsupportedCapabilities
        | RequestPlanningError::OutputLimitExceeded
        | RequestPlanningError::MultimodalInputLimitExceeded
        | RequestPlanningError::ReasoningUnsupported
        | RequestPlanningError::ReasoningLevelUnsupported => api_error(
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            "The selected model does not support the requested capability",
        ),
        RequestPlanningError::UnsupportedParameter(parameter) => typed_api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "unsupported_model_capability",
            "The selected model does not support the requested parameter",
            Some(parameter),
        ),
    }
}

/// Maps Embeddings analysis and preflight failures to the endpoint's exact public error matrix.
pub(super) fn embedding_route_error(error: EmbeddingRequestError) -> Response {
    match error {
        EmbeddingRequestError::InvalidRequest { param } => typed_api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "invalid_request_error",
            "The Embeddings request is invalid",
            param,
        ),
        EmbeddingRequestError::ModelNotFound => typed_api_error(
            StatusCode::NOT_FOUND,
            "invalid_request_error",
            "model_not_found",
            "The requested model does not exist or is not available",
            Some("model"),
        ),
        EmbeddingRequestError::UnsupportedModelCapability { param } => typed_api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "unsupported_model_capability",
            "The selected model does not support the requested Embeddings capability",
            Some(param),
        ),
        EmbeddingRequestError::RouteUnavailable => embedding_server_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "configuration_error",
            "The configured Embeddings route is unavailable",
        ),
    }
}

/// Builds the Embeddings JSON-only media-type rejection before request analysis or egress.
pub(super) fn embedding_unsupported_media_type() -> Response {
    typed_api_error(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "invalid_request_error",
        "unsupported_media_type",
        "Content-Type must be application/json",
        None,
    )
}

/// Builds the Embeddings request hard-limit rejection before any upstream attempt.
pub(super) fn embedding_request_too_large() -> Response {
    typed_api_error(
        StatusCode::PAYLOAD_TOO_LARGE,
        "invalid_request_error",
        "request_too_large",
        "The request body exceeds the configured limit",
        None,
    )
}

/// Builds one endpoint-owned server error without an upstream body or internal topology.
pub(super) fn embedding_server_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> Response {
    typed_api_error(status, "server_error", code, message, None)
}

/// Collapses an Embeddings transport failure into the exact timeout or gateway error contract.
pub(super) fn embedding_upstream_error(error: TransportError) -> Response {
    match error {
        TransportError::Timeout => embedding_server_error(
            StatusCode::GATEWAY_TIMEOUT,
            "upstream_timeout",
            "The upstream request timed out",
        ),
        _ => embedding_server_error(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "The upstream request failed",
        ),
    }
}

/// Replaces an upstream non-success body with a stable error while preserving only safe metadata.
pub(super) fn normalized_embedding_upstream_error(upstream: UpstreamResponse) -> Response {
    // Capture the public status and narrow response metadata before dropping the untrusted body.
    let status = upstream.status();
    let headers = filtered_embedding_error_headers(upstream.headers());
    drop(upstream);

    // Build a gateway-owned envelope and attach only request/rate-limit correlation headers.
    let mut response = embedding_server_error(
        status,
        "upstream_error",
        "The upstream service rejected the request",
    );
    response.headers_mut().extend(headers);
    response
}

/// Keeps only request correlation and rate-limit headers on normalized Embeddings errors.
fn filtered_embedding_error_headers(upstream: &HeaderMap) -> HeaderMap {
    // Exclude upstream media, authentication, cookie, topology, and arbitrary extension headers.
    let mut filtered = HeaderMap::new();
    for (name, value) in upstream {
        let name_text = name.as_str();
        if name == RETRY_AFTER
            || name_text == "openai-request-id"
            || name_text.starts_with("x-ratelimit-")
        {
            filtered.append(name.clone(), value.clone());
        }
    }
    filtered
}

/// Collapses transport failures into timeout or generic gateway errors.
pub(super) fn upstream_error(error: TransportError) -> Response {
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
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    message: &'static str,
    r#type: &'static str,
    param: Option<String>,
    code: &'static str,
}

/// Builds an OpenAI-compatible error envelope without upstream bodies, credentials, or internal topology.
pub(super) fn api_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    api_error_with_param(status, code, message, None)
}

/// Builds a 404 response that does not distinguish internal existence and locates the error at `model`.
pub(super) fn model_not_found() -> Response {
    api_error_with_param(
        StatusCode::NOT_FOUND,
        "model_not_found",
        "The requested model does not exist or is not available",
        Some("model"),
    )
}

/// Builds a normalized error envelope optionally located at a standard request parameter.
fn api_error_with_param(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    param: Option<&str>,
) -> Response {
    typed_api_error(status, "invalid_request_error", code, message, param)
}

/// Serializes one exact OpenAI-compatible error type, code, and optional standard parameter.
fn typed_api_error(
    status: StatusCode,
    error_type: &'static str,
    code: &'static str,
    message: &'static str,
    param: Option<&str>,
) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            error: ErrorBody {
                message,
                r#type: error_type,
                param: param.map(str::to_owned),
                code,
            },
        }),
    )
        .into_response()
}
