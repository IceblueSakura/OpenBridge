//! Bridge 请求顶层字段 allowlist 与 function tool 类型校验。

use serde_json::{Map, Value};

use crate::core::ApiProtocol;

use super::super::BridgeError;

/// 校验请求顶层字段与 tool 类型均属于 Bridge 显式支持范围。
pub(in crate::bridge::conversion) fn reject_unsupported_request(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), BridgeError> {
    // Bridge 与 Native Path 不同：任何未建模顶层字段都必须显式拒绝，不能转换时静默丢失。
    let allowed: &[&str] = match protocol {
        ApiProtocol::ChatCompletions => &[
            "max_completion_tokens",
            "max_tokens",
            "messages",
            "model",
            "parallel_tool_calls",
            "stream",
            "temperature",
            "tool_choice",
            "tools",
            "top_p",
        ],
        ApiProtocol::Responses => &[
            "input",
            "max_output_tokens",
            "model",
            "parallel_tool_calls",
            "stream",
            "temperature",
            "tool_choice",
            "tools",
            "top_p",
        ],
    };
    if source
        .keys()
        .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(BridgeError::UnsupportedSemantics);
    }

    // 拒绝需要状态亲和、异构语义或尚未建模转换策略的顶层字段。
    let unsupported = [
        "background",
        "conversation",
        "include",
        "instructions",
        "metadata",
        "modalities",
        "previous_response_id",
        "reasoning",
        "reasoning_effort",
        "response_format",
        "store",
        "text",
        "truncation",
    ];
    if unsupported.iter().any(|field| {
        source
            .get(*field)
            .is_some_and(|value| !value.is_null() && value != &Value::Bool(false))
    }) {
        return Err(BridgeError::UnsupportedSemantics);
    }
    if protocol == ApiProtocol::ChatCompletions && source.contains_key("functions") {
        return Err(BridgeError::UnsupportedSemantics);
    }

    // 只允许标准 function tool，避免 hosted/custom tool 被降级为普通函数。
    if source
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.get("type").and_then(Value::as_str) != Some("function"))
        })
    {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok(())
}
