//! Bidirectional conversion of non-streaming successful Chat Completions and Responses responses.
//!
//! This module accepts only a single choice or completed response and preserves the existing wire
//! mapping for function-call identities, arguments, usage, and Public Models. Failures and unmodeled
//! terminal states are never presented as success.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::{
    BridgeError,
    shared::{allocate_non_stream_item_id, id_suffix, map_id, required_string, validate_arguments},
};

/// Converts a complete successful Responses object into a single-choice Chat response.
pub(super) fn responses_response_to_chat(
    source: &Map<String, Value>,
    public_model: &str,
    reasoning_supported: bool,
) -> Result<Value, BridgeError> {
    // Project only an explicit completed response to Chat success; other terminal states cannot become stop.
    if source.get("object").and_then(Value::as_str) != Some("response")
        || source.get("status").and_then(Value::as_str) != Some("completed")
    {
        return Err(BridgeError::InvalidShape);
    }
    let output = source
        .get("output")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_calls = Vec::new();
    for item in output {
        let item = item.as_object().ok_or(BridgeError::InvalidShape)?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") => text.push_str(&responses_output_text(item)?),
            Some("reasoning") => {
                if !reasoning_supported {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                reasoning.push_str(&responses_reasoning_text(item)?);
            }
            Some("function_call") => {
                let arguments = required_string(item, "arguments")?;
                validate_arguments(&arguments)?;
                tool_calls.push(json!({
                    "function": {
                        "arguments": arguments,
                        "name": required_string(item, "name")?
                    },
                    "id": required_string(item, "call_id")?,
                    "type": "function"
                }));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }

    // Build the single-choice Chat response and map usage names.
    let upstream_id = required_string(source, "id")?;
    let mut message = Map::new();
    message.insert("role".to_owned(), Value::String("assistant".to_owned()));
    if !reasoning.is_empty() {
        message.insert(
            "reasoning_content".to_owned(),
            Value::String(reasoning.clone()),
        );
    }
    let finish_reason = if tool_calls.is_empty() {
        message.insert(
            "content".to_owned(),
            if text.is_empty() && !reasoning.is_empty() {
                Value::Null
            } else {
                Value::String(text)
            },
        );
        "stop"
    } else {
        message.insert(
            "content".to_owned(),
            if text.is_empty() {
                Value::Null
            } else {
                Value::String(text)
            },
        );
        message.insert("tool_calls".to_owned(), Value::Array(tool_calls));
        "tool_calls"
    };
    let mut result = json!({
        "choices": [{"finish_reason": finish_reason, "index": 0, "message": message}],
        "id": map_id(&upstream_id, "resp_", "chatcmpl_"),
        "model": public_model,
        "object": "chat.completion"
    });
    if let Some(usage) = source.get("usage").and_then(Value::as_object) {
        result["usage"] = json!({
            "completion_tokens": usage.get("output_tokens").cloned().unwrap_or(Value::Null),
            "prompt_tokens": usage.get("input_tokens").cloned().unwrap_or(Value::Null),
            "total_tokens": usage.get("total_tokens").cloned().unwrap_or(Value::Null)
        });
    }
    Ok(result)
}

/// Extracts supported output text from a Responses message item.
fn responses_output_text(item: &Map<String, Value>) -> Result<String, BridgeError> {
    // Read Responses message content parts and reject unmodeled output types.
    let parts = item
        .get("content")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let mut text = String::new();
    // Merge each confirmed output_text part in wire order.
    for part in parts {
        let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
        if part.get("type").and_then(Value::as_str) != Some("output_text") {
            return Err(BridgeError::UnsupportedSemantics);
        }
        text.push_str(&required_string(part, "text")?);
    }
    Ok(text)
}

/// Extracts readable content/summary while discarding validated output-only opaque continuation.
fn responses_reasoning_text(item: &Map<String, Value>) -> Result<String, BridgeError> {
    validate_encrypted_reasoning(item)?;
    let mut text = String::new();
    for (field, expected_type) in [("content", "reasoning_text"), ("summary", "summary_text")] {
        let Some(parts) = item.get(field) else {
            continue;
        };
        let parts = parts.as_array().ok_or(BridgeError::InvalidShape)?;
        for part in parts {
            let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
            if part.get("type").and_then(Value::as_str) != Some(expected_type) {
                return Err(BridgeError::UnsupportedSemantics);
            }
            text.push_str(&required_string(part, "text")?);
        }
    }
    Ok(text)
}

/// Validates the Provider-bound continuation shape before omitting it from stateless Chat output.
fn validate_encrypted_reasoning(item: &Map<String, Value>) -> Result<(), BridgeError> {
    match item.get("encrypted_content") {
        None | Some(Value::Null | Value::String(_)) => Ok(()),
        Some(_) => Err(BridgeError::InvalidShape),
    }
}

/// Converts a complete successful single-choice Chat response into a Responses object.
pub(super) fn chat_response_to_responses(
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
