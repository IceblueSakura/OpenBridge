//! 将 OpenAI-compatible 请求解析为固定 snapshot 下的原生上游 candidate。
//!
//! pipeline 不转换 Chat/Responses 语义：它只验证 JSON、提取 public `model` 与请求实际
//! 使用的 capability，然后为每个兼容 deployment 复制请求并替换 `model`。这使 ingress
//! 能在不丢字段的前提下选择或回退 candidate，同时保留 provider-bound continuation 的
//! 亲和性约束。

use bytes::Bytes;
use serde_json::Value;
use thiserror::Error;

use crate::{
    config::RegistrySnapshot,
    core::{CapabilitySet, Protocol, ValidatedRequest},
};

#[derive(Debug, Error)]
pub enum RouteError {
    #[error("request body must be a JSON object")]
    InvalidJson,
    #[error("request body must contain a non-empty model")]
    MissingModel,
    #[error("requested model is not configured")]
    UnknownModel,
    #[error("configured model has no deployment candidate")]
    NoDeployment,
    #[error("selected deployment does not support this protocol")]
    UnsupportedProtocol,
    #[error("selected deployment does not support streaming")]
    StreamingUnsupported,
    #[error("selected deployment does not support requested capabilities")]
    UnsupportedCapabilities,
}

/// 已完成 alias/capability 解析的原生请求。
///
/// candidates 保持 route 配置顺序；`allows_fallback` 不是一般性的重试开关，而是保护
/// `previous_response_id` 等 provider-issued opaque state 不被重放到其他 deployment。
#[derive(Debug)]
pub struct PreparedNativeRequest {
    candidates: Vec<PreparedNativeCandidate>,
    is_streaming: bool,
    allows_fallback: bool,
}

#[derive(Debug)]
pub struct PreparedNativeCandidate {
    deployment_id: String,
    request: ValidatedRequest,
}

impl PreparedNativeRequest {
    pub fn deployment_id(&self) -> &str {
        self.primary().deployment_id()
    }

    pub fn request(&self) -> &ValidatedRequest {
        self.primary().request()
    }

    pub fn candidates(&self) -> &[PreparedNativeCandidate] {
        &self.candidates
    }

    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    pub fn allows_fallback(&self) -> bool {
        self.allows_fallback
    }

    pub fn into_request(self) -> ValidatedRequest {
        self.candidates
            .into_iter()
            .next()
            .expect("prepared request always has a candidate")
            .request
    }

    fn primary(&self) -> &PreparedNativeCandidate {
        self.candidates
            .first()
            .expect("prepared request always has a candidate")
    }
}

impl PreparedNativeCandidate {
    pub fn deployment_id(&self) -> &str {
        &self.deployment_id
    }

    pub fn request(&self) -> &ValidatedRequest {
        &self.request
    }
}

/// 解析 public alias，并生成每个兼容 deployment 的原生请求副本。
///
/// 请求字段除 `model` 外保持原样。capability gate 在这里完成，确保不支持的 tools、
/// structured output、background/store 或 continuation 请求不会被静默删字段后发往
/// upstream。若前序 candidate 不兼容，保留第一个确定错误以给客户端稳定的 4xx 语义。
pub fn prepare_native_request(
    snapshot: &RegistrySnapshot,
    protocol: Protocol,
    body: Bytes,
) -> Result<PreparedNativeRequest, RouteError> {
    let document: Value = serde_json::from_slice(&body).map_err(|_| RouteError::InvalidJson)?;
    let object = document.as_object().ok_or(RouteError::InvalidJson)?;
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .ok_or(RouteError::MissingModel)?;
    let is_streaming = object.get("stream").and_then(Value::as_bool) == Some(true);
    let requested_features = CapabilitySet {
        chat: false,
        responses: false,
        streaming: false,
        function_tools: object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty()),
        structured_output: requests_structured_output(object),
        previous_response_id: object
            .get("previous_response_id")
            .is_some_and(|value| !value.is_null()),
        background: object.get("background").and_then(Value::as_bool) == Some(true),
        response_store: object.get("store").and_then(Value::as_bool) == Some(true),
    };
    let candidates = snapshot
        .alias(public_model)
        .ok_or(RouteError::UnknownModel)?
        .candidates();
    let mut first_error = None;
    let mut prepared_candidates = Vec::new();
    for deployment_id in candidates {
        let deployment = snapshot
            .deployment(deployment_id)
            .ok_or(RouteError::NoDeployment)?;
        if let Some(error) = candidate_error(
            protocol,
            object,
            requested_features,
            deployment.capabilities(),
        ) {
            first_error.get_or_insert(error);
            continue;
        }
        let mut candidate_document = document.clone();
        candidate_document
            .as_object_mut()
            .expect("request document was validated as an object")
            .insert(
                "model".to_owned(),
                Value::String(deployment.upstream_model().to_owned()),
            );
        let candidate_body = serde_json::to_vec(&candidate_document)
            .map(Bytes::from)
            .map_err(|_| RouteError::InvalidJson)?;
        prepared_candidates.push(PreparedNativeCandidate {
            deployment_id: deployment_id.clone(),
            request: ValidatedRequest::new(protocol, candidate_body),
        });
    }
    if prepared_candidates.is_empty() {
        return Err(first_error.unwrap_or(RouteError::NoDeployment));
    }

    Ok(PreparedNativeRequest {
        candidates: prepared_candidates,
        is_streaming,
        allows_fallback: !requested_features.previous_response_id,
    })
}

fn candidate_error(
    protocol: Protocol,
    object: &serde_json::Map<String, Value>,
    requested_features: CapabilitySet,
    capabilities: &CapabilitySet,
) -> Option<RouteError> {
    let protocol_supported = match protocol {
        Protocol::ChatCompletions => capabilities.chat,
        Protocol::Responses => capabilities.responses,
    };
    if !protocol_supported {
        return Some(RouteError::UnsupportedProtocol);
    }
    if object.get("stream").and_then(Value::as_bool) == Some(true) && !capabilities.streaming {
        return Some(RouteError::StreamingUnsupported);
    }
    if !requested_features.is_subset_of(*capabilities) {
        return Some(RouteError::UnsupportedCapabilities);
    }
    None
}

fn requests_structured_output(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("response_format")
        .is_some_and(is_non_text_format)
        || object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format"))
            .is_some_and(is_non_text_format)
}

fn is_non_text_format(format: &Value) -> bool {
    format
        .as_object()
        .and_then(|format| format.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|format_type| format_type != "text")
}
