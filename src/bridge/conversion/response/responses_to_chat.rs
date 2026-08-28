//! Converts complete Responses success objects into single-choice Chat responses.

use serde_json::{Map, Value, json};

use super::super::{
    BridgeError,
    shared::{map_id, required_string, validate_arguments},
    usage::responses_usage_to_chat,
};

/// Converts a complete successful Responses object into a single-choice Chat response.
pub(in crate::bridge::conversion) fn responses_response_to_chat(
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
    if let Some(usage) = source.get("usage").filter(|usage| !usage.is_null()) {
        result["usage"] =
            responses_usage_to_chat(usage.as_object().ok_or(BridgeError::InvalidShape)?)?;
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
