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
    core::{Protocol, ProtocolCapabilities, ValidatedRequest},
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
    #[error("requested maximum output exceeds the configured model limit")]
    OutputLimitExceeded,
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

/// 单次请求实际使用的能力。它不等同于 deployment 配置：`protocol` 是两个端点共享的
/// 需求，Responses 专有状态单独保留，避免被误用于 Chat Completions 路由。
#[derive(Clone, Copy, Debug)]
struct RequestedCapabilities {
    protocol: ProtocolCapabilities,
    unmodeled_tools: bool,
    previous_response_id: bool,
    background: bool,
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
    let requested_output_tokens = requested_output_tokens(object);
    let requests_function_calling = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(is_function_tool));
    let requests_unmodeled_tools = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(|tool| !is_function_tool(tool)));
    let requested_features = RequestedCapabilities {
        protocol: ProtocolCapabilities {
            enabled: false,
            streaming: is_streaming,
            function_calling: requests_function_calling,
            parallel_tool_calls: requests_function_calling
                && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
            image_input: requests_image_input(protocol, object),
            structured_outputs: requests_structured_outputs(object),
            store: object.get("store").and_then(Value::as_bool) == Some(true),
        },
        unmodeled_tools: requests_unmodeled_tools,
        previous_response_id: protocol == Protocol::Responses
            && object
                .get("previous_response_id")
                .is_some_and(|value| !value.is_null()),
        background: protocol == Protocol::Responses
            && object.get("background").and_then(Value::as_bool) == Some(true),
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
            requested_features,
            deployment.capabilities(),
            deployment.model_limits().max_output_tokens(),
            requested_output_tokens,
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
    requested_features: RequestedCapabilities,
    capabilities: &crate::core::CapabilitySet,
    configured_max_output_tokens: Option<u32>,
    requested_output_tokens: Option<u64>,
) -> Option<RouteError> {
    let protocol_capabilities = match protocol {
        Protocol::ChatCompletions => capabilities.chat_completions,
        Protocol::Responses => capabilities.responses.protocol_capabilities(),
    };
    if !protocol_capabilities.enabled {
        return Some(RouteError::UnsupportedProtocol);
    }
    if requested_features.unmodeled_tools {
        return Some(RouteError::UnsupportedCapabilities);
    }
    if requested_features.protocol.streaming && !protocol_capabilities.streaming {
        return Some(RouteError::StreamingUnsupported);
    }
    if !requested_features
        .protocol
        .is_subset_of(protocol_capabilities)
    {
        return Some(RouteError::UnsupportedCapabilities);
    }
    if protocol == Protocol::Responses
        && ((requested_features.previous_response_id
            && !capabilities.responses.previous_response_id)
            || (requested_features.background && !capabilities.responses.background))
    {
        return Some(RouteError::UnsupportedCapabilities);
    }
    if configured_max_output_tokens.is_some_and(|limit| {
        requested_output_tokens.is_some_and(|requested| requested > u64::from(limit))
    }) {
        return Some(RouteError::OutputLimitExceeded);
    }
    None
}

/// 仅识别 OpenAI-compatible message/input item 内的 image content part，而不尝试在热路径计算 token。
///
/// `image_url`（Chat）和 `input_image`（Responses）是协议字段；未知 part 会被原生
/// 透传，因此不能依据其他任意 JSON 中同名 `type` 推测视觉能力。
fn requests_image_input(protocol: Protocol, object: &serde_json::Map<String, Value>) -> bool {
    match protocol {
        Protocol::ChatCompletions => object
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| content_contains_part_type(message.get("content"), "image_url"))
            }),
        Protocol::Responses => object
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("type").and_then(Value::as_str) == Some("input_image")
                        || content_contains_part_type(item.get("content"), "input_image")
                })
            }),
    }
}

fn content_contains_part_type(content: Option<&Value>, expected_type: &str) -> bool {
    content.and_then(Value::as_array).is_some_and(|parts| {
        parts
            .iter()
            .any(|part| part.get("type").and_then(Value::as_str) == Some(expected_type))
    })
}

/// `function_calling` 只覆盖 OpenAI 的 JSON-schema function tools。Built-in 和 custom
/// tools 需要各自的配置语义与 probe，尚未建模时不得借由 `tools[]` 原生透传而被误认为
/// 已支持。
fn is_function_tool(tool: &Value) -> bool {
    tool.get("type").and_then(Value::as_str) == Some("function")
}

/// Chat 兼容面存在新旧两个输出上限字段；当客户端同时给出多个字段时，选择其中最大的
/// 值做本地上界校验，绝不悄悄改写上游请求。字段值不是非负整数时仍交由上游协议验证。
fn requested_output_tokens(object: &serde_json::Map<String, Value>) -> Option<u64> {
    ["max_output_tokens", "max_completion_tokens", "max_tokens"]
        .iter()
        .filter_map(|field| object.get(*field).and_then(Value::as_u64))
        .max()
}

fn requests_structured_outputs(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("response_format")
        .is_some_and(is_non_text_format)
        || object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format"))
            .is_some_and(is_non_text_format)
        || object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| tools.iter().any(tool_requests_strict_mode))
}

/// Function strict mode is specified inside `function` for Chat Completions and directly on a
/// function tool in Responses. The function-calling guide defines strict mode in terms of
/// Structured Outputs, so either wire shape requires `structured_outputs`.
fn tool_requests_strict_mode(tool: &Value) -> bool {
    tool.get("strict").and_then(Value::as_bool) == Some(true)
        || tool
            .get("function")
            .and_then(Value::as_object)
            .and_then(|function| function.get("strict"))
            .and_then(Value::as_bool)
            == Some(true)
}

fn is_non_text_format(format: &Value) -> bool {
    format
        .as_object()
        .and_then(|format| format.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|format_type| format_type != "text")
}
