//! Bridge request top-level allowlist and function-tool type validation.

use serde_json::{Map, Value};

use crate::core::ApiProtocol;

use super::super::BridgeError;

/// Validates that request fields and tool types are explicitly supported by the Bridge.
pub(in crate::bridge::conversion) fn reject_unsupported_request(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), BridgeError> {
    // Unlike the Native path, the Bridge must explicitly reject every unmodeled top-level field instead of dropping it during conversion.
    let allowed: &[&str] = match protocol {
        ApiProtocol::ChatCompletions => &[
            "max_completion_tokens",
            "max_tokens",
            "messages",
            "model",
            "parallel_tool_calls",
            "reasoning_effort",
            "response_format",
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
            "reasoning",
            "stream",
            "text",
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

    // Reject top-level fields that require state affinity, heterogeneous semantics, or an unmodeled conversion strategy.
    let unsupported = [
        "background",
        "conversation",
        "include",
        "instructions",
        "metadata",
        "modalities",
        "previous_response_id",
        "store",
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

    // Allow only standard function tools so hosted or custom tools cannot be downgraded to ordinary functions.
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
