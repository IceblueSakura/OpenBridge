//! Converts complete single-choice Chat success objects into Responses objects.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::super::{
    BridgeError,
    shared::{allocate_non_stream_item_id, id_suffix, required_string, validate_arguments},
};

/// Converts a complete successful single-choice Chat response into a Responses object.
pub(in crate::bridge::conversion) fn chat_response_to_responses(
    source: &Map<String, Value>,
    public_model: &str,
    reasoning_supported: bool,
) -> Result<Value, BridgeError> {
    // Accept only one completed choice to avoid undefined ordering when merging multiple choices.
    let choices = source
        .get("choices")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    if choices.len() != 1 {
        return Err(BridgeError::UnsupportedSemantics);
    }
    let choice = choices[0].as_object().ok_or(BridgeError::InvalidShape)?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or(BridgeError::InvalidShape)?;
    let upstream_id = required_string(source, "id")?;
    if !matches!(
        choice.get("finish_reason").and_then(Value::as_str),
        Some("stop" | "tool_calls")
    ) {
        return Err(BridgeError::UnsupportedSemantics);
    }

    // Build Responses output from the choice content and assign stable unique item IDs to parallel tool calls.
    let suffix = id_suffix(&upstream_id, "chatcmpl_");
    let mut output = Vec::new();
    let mut item_ids = BTreeSet::new();
    if let Some(reasoning) = message
        .get("reasoning_content")
        .filter(|value| !value.is_null() && *value != &Value::String(String::new()))
    {
        if !reasoning_supported {
            return Err(BridgeError::UnsupportedSemantics);
        }
        let reasoning = reasoning.as_str().ok_or(BridgeError::InvalidShape)?;
        if !reasoning.is_empty() {
            output.push(json!({
                "content": [{"text": reasoning, "type": "reasoning_text"}],
                "id": format!("rs_{suffix}"),
                "status": "completed",
                "summary": [],
                "type": "reasoning"
            }));
        }
    }
    if let Some(content) = message.get("content").and_then(Value::as_str)
        && !content.is_empty()
    {
        output.push(json!({
            "content": [{"annotations": [], "text": content, "type": "output_text"}],
            "id": format!("msg_{suffix}"),
            "role": "assistant",
            "status": "completed",
            "type": "message"
        }));
    }
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (ordinal, call) in calls.iter().enumerate() {
            let call = call.as_object().ok_or(BridgeError::InvalidShape)?;
            let function = call
                .get("function")
                .and_then(Value::as_object)
                .ok_or(BridgeError::InvalidShape)?;
            let arguments = required_string(function, "arguments")?;
            validate_arguments(&arguments)?;
            let call_id = required_string(call, "id")?;
            let item_id = allocate_non_stream_item_id(&call_id, ordinal + 1, &mut item_ids);
            output.push(json!({
                "arguments": arguments,
                "call_id": call_id,
                "id": item_id,
                "name": required_string(function, "name")?,
                "status": "completed",
                "type": "function_call"
            }));
        }
    }
    if output.is_empty() {
        return Err(BridgeError::InvalidShape);
    }

    // Map response identity, Public Model, and usage fields before returning the complete success object.
    let mut result = json!({
        "id": format!("resp_{suffix}"),
        "model": public_model,
        "object": "response",
        "output": output,
        "status": "completed"
    });
    if let Some(usage) = source.get("usage").and_then(Value::as_object) {
        result["usage"] = json!({
            "input_tokens": usage.get("prompt_tokens").cloned().unwrap_or(Value::Null),
            "output_tokens": usage.get("completion_tokens").cloned().unwrap_or(Value::Null),
            "total_tokens": usage.get("total_tokens").cloned().unwrap_or(Value::Null)
        });
    }
    Ok(result)
}
