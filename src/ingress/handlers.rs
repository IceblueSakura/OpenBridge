//! OpenAI-compatible endpoint 与健康检查 handler。

use axum::{Extension, Json, extract::State, response::Response};
use bytes::Bytes;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use serde::Serialize;

use crate::{core::ApiProtocol, observability::RequestObservation};

use super::{forwarding::forward_request, response::api_error, state::GatewayState};

/// 只从不可变 registry 生成 Public Model 列表，不暴露上游模型或 target。
pub(super) async fn models(State(state): State<GatewayState>) -> Json<ModelListResponse> {
    // 读取稳定的 Public Model id 并构造 OpenAI-compatible list envelope。
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
pub(super) struct ModelListResponse {
    object: &'static str,
    data: Vec<PublicModel>,
}

#[derive(Serialize)]
struct PublicModel {
    id: String,
    object: &'static str,
    owned_by: &'static str,
}

/// 接收 Chat Completions JSON 请求，并交给统一 forwarding pipeline。
pub(super) async fn chat_completions(
    State(state): State<GatewayState>,
    Extension(observation): Extension<RequestObservation>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 先校验原生 JSON media type，再进入统一 route/egress pipeline。
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_request(
        state,
        observation,
        ApiProtocol::ChatCompletions,
        headers,
        body,
    )
    .await
}

/// 接收 Responses JSON 请求，并交给统一 forwarding pipeline。
pub(super) async fn responses(
    State(state): State<GatewayState>,
    Extension(observation): Extension<RequestObservation>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 先校验原生 JSON media type，再进入统一 route/egress pipeline。
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_request(state, observation, ApiProtocol::Responses, headers, body).await
}

/// 判断请求是否恰好携带 application/json media type。
fn has_json_content_type(headers: &HeaderMap) -> bool {
    // 拒绝缺失或重复 Content-Type，避免多个值造成边界解释不一致。
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    // 忽略 media type 参数，但只接受 application/json。
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

/// 为非 JSON 请求生成稳定的协议错误响应。
fn unsupported_media_type() -> Response {
    api_error(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "invalid_content_type",
        "Content-Type must be application/json",
    )
}

#[derive(Serialize)]
pub(super) struct HealthResponse {
    status: &'static str,
    registry_version: String,
}

/// 返回本地服务健康状态和当前编译 registry 版本。
pub(super) async fn health(State(state): State<GatewayState>) -> Json<HealthResponse> {
    // 仅报告服务存活和编译期 registry 版本，不探测真实 Provider。
    Json(HealthResponse {
        status: "ok",
        registry_version: state.registry.version().as_str().to_owned(),
    })
}
