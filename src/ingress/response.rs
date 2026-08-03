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

use crate::{pipeline::RequestPlanningError, transport::upstream::TransportError};

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
        | RequestPlanningError::InvalidReasoningConfiguration => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "Request body is invalid",
        ),
        RequestPlanningError::UnknownModel | RequestPlanningError::NoRoute => model_not_found(),
        RequestPlanningError::UnimplementedCapabilities => api_error(
            StatusCode::BAD_REQUEST,
            "unimplemented_request",
            "The request uses a capability that is not implemented",
        ),
        RequestPlanningError::UnsupportedProtocol
        | RequestPlanningError::StreamingUnsupported
        | RequestPlanningError::UnsupportedCapabilities
        | RequestPlanningError::OutputLimitExceeded
        | RequestPlanningError::ReasoningUnsupported
        | RequestPlanningError::ReasoningLevelUnsupported => api_error(
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            "The selected model does not support the requested capability",
        ),
    }
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
    param: Option<&'static str>,
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
    param: Option<&'static str>,
) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            error: ErrorBody {
                message,
                r#type: "invalid_request_error",
                param,
                code,
            },
        }),
    )
        .into_response()
}
