//! 已选定上游响应的 Native/Bridged body 接管与安全下游构造。
//!
//! 本模块在 retry/fallback 已结束后处理 status、响应头、SSE 校验和非流式 Bridge；错误
//! response body 保持原样，且一旦接管 body 就不会再拼接其他上游 attempt。

use axum::{body::to_bytes, response::Response};
use http::{StatusCode, header::CONTENT_TYPE};

use crate::{
    bridge::BridgePlan, core::ApiProtocol, observability::RequestObservation,
    provider::ProviderAdapter, transport::upstream::UpstreamResponse,
};

use super::super::{
    response::{api_error, filtered_upstream_headers},
    streaming::{bridge_sse_body, validate_sse_body},
};

/// 一次已选定候选的响应转换、SSE 和观测上下文。
pub(super) struct UpstreamResponseContext {
    pub(super) validate_sse: bool,
    pub(super) protocol: ApiProtocol,
    pub(super) adapter: ProviderAdapter,
    pub(super) max_sse_event_bytes: usize,
    pub(super) max_json_body_bytes: usize,
    pub(super) bridge: Option<BridgePlan>,
    pub(super) observation: RequestObservation,
}

/// 将上游 status、安全响应头和 Native/Bridged body 交给下游。
///
/// SSE 仅在原请求要求 streaming、上游返回成功状态且 `Content-Type` 确为
/// `text/event-stream` 时验证。错误响应即使对应 streaming request 也可能是 JSON 或其他
/// 诊断 body；对其做 SSE 解码会破坏可见的 HTTP 错误语义。
pub(super) async fn upstream_response(
    upstream: UpstreamResponse,
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

    // 构造保留安全上游 headers 的最终下游 response。
    let mut response = Response::builder()
        .status(status)
        .body(body)
        .expect("valid upstream response status");
    response.headers_mut().extend(response_headers);
    response
}
