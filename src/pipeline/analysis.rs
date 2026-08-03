//! 从 OpenAI-compatible JSON 中提取与 registry 无关的请求事实。

use bytes::Bytes;
use serde_json::Value;

use crate::{
    core::{ApiProtocol, EndpointCapabilities, ReasoningOutput},
    registry::ReasoningLevel,
};

use super::{
    error::RequestPlanningError,
    types::{RequestRequirements, RequestedCapabilities, RequestedReasoning},
};

/// 解析下游请求并提取独立于 registry 的请求事实。
///
/// 此阶段不选择 route，也不改写请求正文。
pub fn analyze_request(
    protocol: ApiProtocol,
    body: &Bytes,
) -> Result<RequestRequirements, RequestPlanningError> {
    // 解析 JSON object 并提取 public model 与 stream 标志。
    let document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object()
        .ok_or(RequestPlanningError::InvalidJson)?;
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .ok_or(RequestPlanningError::MissingModel)?;
    let is_streaming = object.get("stream").and_then(Value::as_bool) == Some(true);
    // 根据协议字段推导请求实际使用的 capability。
    let requested_output_tokens = requested_output_tokens(object);
    let requests_function_calling = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(is_function_tool));
    let requests_unmodeled_tools = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(|tool| !is_function_tool(tool)));
    let requested_capabilities = RequestedCapabilities {
        protocol: EndpointCapabilities {
            enabled: false,
            streaming: is_streaming,
            function_calling: requests_function_calling,
            parallel_tool_calls: requests_function_calling
                && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
            image_input: requests_image_input(protocol, object),
            structured_outputs: requests_structured_outputs(object),
            store: object.get("store").and_then(Value::as_bool) == Some(true),
            reasoning_output: ReasoningOutput::Unknown,
        },
        unmodeled_tools: requests_unmodeled_tools,
        reasoning: requested_reasoning(protocol, object),
        previous_response_id: protocol == ApiProtocol::Responses
            && object
                .get("previous_response_id")
                .is_some_and(|value| !value.is_null()),
        background: protocol == ApiProtocol::Responses
            && object.get("background").and_then(Value::as_bool) == Some(true),
    };
    // 固化请求事实，后续 route 规划不再重新解释 body。
    Ok(RequestRequirements {
        public_model: public_model.to_owned(),
        protocol,
        is_streaming,
        requested_output_tokens,
        requested_capabilities,
    })
}

/// 仅识别 OpenAI-compatible message/input item 内的 image content part，而不尝试在热路径计算 token。
///
/// `image_url`（Chat）和 `input_image`（Responses）是协议字段；未知 part 会被原生
/// 透传，因此不能依据其他任意 JSON 中同名 `type` 推测视觉能力。
fn requests_image_input(protocol: ApiProtocol, object: &serde_json::Map<String, Value>) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => object
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| content_contains_part_type(message.get("content"), "image_url"))
            }),
        ApiProtocol::Responses => {
            object
                .get("input")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("input_image")
                            || content_contains_part_type(item.get("content"), "input_image")
                    })
                })
        }
    }
}

/// 判断 content 数组中是否存在指定协议 part type。
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

/// 按协议读取标准 reasoning 配置；没有该字段时不得据模型目录推测调用方需要 reasoning。
fn requested_reasoning(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> RequestedReasoning {
    // Responses 只接受标准 reasoning 对象，拒绝 Chat 的顶层 shorthand。
    if protocol == ApiProtocol::Responses && object.contains_key("reasoning_effort") {
        return RequestedReasoning::UnknownLevel;
    }

    // Chat 只接受标准 reasoning_effort，拒绝 Responses 的 reasoning 对象。
    if protocol == ApiProtocol::ChatCompletions && object.contains_key("reasoning") {
        return if object.contains_key("reasoning_effort") {
            RequestedReasoning::Conflicting
        } else {
            RequestedReasoning::UnknownLevel
        };
    }

    // 分别读取当前协议的标准字段，后续统一检查是否歧义。
    let shorthand_value = object
        .get("reasoning_effort")
        .filter(|value| !value.is_null());
    let shorthand = shorthand_value
        .and_then(Value::as_str)
        .and_then(ReasoningLevel::from_wire);
    let reasoning_value = object.get("reasoning").filter(|value| !value.is_null());
    let object_effort = reasoning_value
        .and_then(Value::as_object)
        .and_then(|reasoning| reasoning.get("effort"));
    let object_level = object_effort
        .and_then(Value::as_str)
        .and_then(ReasoningLevel::from_wire);

    // 同时出现两个配置来源时必须相同，否则在 Native/Bridge 两条路径都 fail closed。
    if shorthand_value.is_some() && reasoning_value.is_some() {
        return match (shorthand, object_level) {
            (Some(left), Some(right)) if left == right => RequestedReasoning::Level(left),
            (Some(_), Some(_)) => RequestedReasoning::Conflicting,
            _ => RequestedReasoning::UnknownLevel,
        };
    }
    if shorthand_value.is_some() {
        return shorthand_value
            .and_then(Value::as_str)
            .and_then(ReasoningLevel::from_wire)
            .map(RequestedReasoning::Level)
            .unwrap_or(RequestedReasoning::UnknownLevel);
    }
    let Some(reasoning) = reasoning_value else {
        return RequestedReasoning::None;
    };
    // 没有 shorthand 时读取 Responses reasoning 对象的 effort。
    let Some(effort) = reasoning
        .as_object()
        .and_then(|reasoning| reasoning.get("effort"))
    else {
        return if reasoning.is_object() {
            RequestedReasoning::Unspecified
        } else {
            RequestedReasoning::UnknownLevel
        };
    };
    // 将已知 wire level 映射为内部枚举，未知值保持 fail closed。
    effort
        .as_str()
        .and_then(ReasoningLevel::from_wire)
        .map(RequestedReasoning::Level)
        .unwrap_or(RequestedReasoning::UnknownLevel)
}

/// 识别 response format、text format 或 strict function tool 对结构化输出的请求。
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

/// Chat Completions 将 strict 放在 `function` 内，Responses 则直接放在 function tool 上。
/// 两种 wire 形状都属于 Structured Outputs 语义，因此都需要 `structured_outputs` 能力。
fn tool_requests_strict_mode(tool: &Value) -> bool {
    tool.get("strict").and_then(Value::as_bool) == Some(true)
        || tool
            .get("function")
            .and_then(Value::as_object)
            .and_then(|function| function.get("strict"))
            .and_then(Value::as_bool)
            == Some(true)
}

/// 判断 format object 是否显式要求非纯文本输出。
fn is_non_text_format(format: &Value) -> bool {
    format
        .as_object()
        .and_then(|format| format.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|format_type| format_type != "text")
}
