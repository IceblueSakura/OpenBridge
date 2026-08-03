//! 下游安全响应头与统一错误响应。

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

/// 仅透传 OpenAI-compatible client 需要且不会改变 proxy 安全边界的上游响应头。
///
/// 不透传 cookie、认证、连接管理或任意自定义 header；这样上游无法借 proxy 向客户端设置
/// 会话状态，也不会泄露内部 transport 细节。
pub(super) fn filtered_upstream_headers(upstream: &HeaderMap) -> HeaderMap {
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

/// 将请求规划错误映射为稳定的下游 HTTP 错误，不泄露内部 route 详情。
pub(super) fn route_error(error: RequestPlanningError) -> Response {
    match error {
        RequestPlanningError::InvalidJson
        | RequestPlanningError::MissingModel
        | RequestPlanningError::InvalidReasoningConfiguration => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "Request body is invalid",
        ),
        RequestPlanningError::UnknownModel | RequestPlanningError::NoRoute => api_error(
            StatusCode::NOT_FOUND,
            "model_not_found",
            "The requested model is not available",
        ),
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
            "unsupported_request",
            "The selected model does not support this request",
        ),
    }
}

/// 将 transport 失败收敛为 timeout 或通用 gateway error。
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

/// 构造不包含上游正文、凭证或内部拓扑的 OpenAI-compatible error envelope。
pub(super) fn api_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
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
