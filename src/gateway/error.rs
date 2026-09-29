//! Stable client errors never include upstream bodies, URLs, credentials or parser input.
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
#[derive(Clone, Copy, Debug)]
pub(super) struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
}
impl ApiError {
    pub const fn new(status: StatusCode, code: &'static str) -> Self {
        Self { status, code }
    }
    pub const fn invalid() -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request")
    }
    pub const fn upstream() -> Self {
        Self::new(StatusCode::BAD_GATEWAY, "upstream_error")
    }
    pub const fn timeout() -> Self {
        Self::new(StatusCode::GATEWAY_TIMEOUT, "upstream_timeout")
    }
    pub const fn shutdown() -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "shutting_down")
    }
    pub fn status(status: u16) -> Self {
        if status == 429 {
            Self::new(StatusCode::TOO_MANY_REQUESTS, "upstream_rate_limit")
        } else {
            Self::upstream()
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let kind = if self.status.is_client_error() {
            "invalid_request_error"
        } else {
            "api_error"
        };
        let mut response=(self.status,Json(json!({"error":{"message":"The request could not be completed.","type":kind,"param":null,"code":self.code}}))).into_response();
        response.headers_mut().insert(
            "cache-control",
            axum::http::HeaderValue::from_static("no-store"),
        );
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                "www-authenticate",
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}
