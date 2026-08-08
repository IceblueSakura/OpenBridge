//! Restricted conversion from Chat Completions requests to Responses requests.
//!
//! This module converts message content, function schemas, and tool choices, and maintains a local
//! identity ledger from Chat tool calls/results to Responses input items.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::super::{
    BridgeError,
    shared::{allocate_non_stream_item_id, copy_fields, required_string, validate_arguments},
};
use super::structured::chat_response_format_to_responses;

/// Converts a Chat request that passed top-level validation into a Responses request.
pub(in crate::bridge::conversion) fn chat_request_to_responses(
    source: &Map<String, Value>,
    upstream_model: &str,
    reasoning_supported: bool,
    reasoning_summary: bool,
) -> Result<Value, BridgeError> {
    // Convert Chat messages and validate the local tool call/result identity ledger.
    let messages = source
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let stream = source.get("stream").and_then(Value::as_bool) == Some(true);
    let tools_present = source.get("tools").is_some();
    let input = chat_messages_to_responses(messages, stream, tools_present, reasoning_supported)?;

    // Copy fields shared by both protocols and convert the function schema and output-token fields.
    let mut result = Map::new();
    result.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    result.insert("input".to_owned(), input);
    result.insert("stream".to_owned(), Value::Bool(stream));
    copy_fields(
        source,
        &mut result,
        &["parallel_tool_calls", "temperature", "top_p"],
    );
    if let Some(effort) = chat_reasoning_effort(source)? {
        if !reasoning_supported && effort != "none" {
            return Err(BridgeError::UnsupportedSemantics);
        }
        let mut reasoning = json!({"effort": effort});
        if reasoning_summary && effort != "none" {
            reasoning["summary"] = Value::String("auto".to_owned());
        }
        result.insert("reasoning".to_owned(), reasoning);
    }
    if let Some(max_tokens) = source
        .get("max_completion_tokens")
        .or_else(|| source.get("max_tokens"))
    {
        result.insert("max_output_tokens".to_owned(), max_tokens.clone());
    }
    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        result.insert(
            "tools".to_owned(),
            Value::Array(
                tools
                    .iter()
                    .map(chat_tool_to_responses)
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(tool_choice) = source.get("tool_choice") {
        result.insert(
            "tool_choice".to_owned(),
            chat_tool_choice_to_responses(tool_choice)?,
        );
    }
    if let Some(response_format) = source
        .get("response_format")
        .filter(|value| !value.is_null())
    {
        result.insert(
            "text".to_owned(),
            chat_response_format_to_responses(response_format)?,
        );
    }
    Ok(Value::Object(result))
}

/// Converts ordered Chat messages into Responses input and validates call/result identities.
fn chat_messages_to_responses(
    messages: &[Value],
    stream: bool,
    tools_present: bool,
    reasoning_supported: bool,
) -> Result<Value, BridgeError> {
    // Preserve the Responses shorthand for the common single streaming-text request; use explicit input items for other history.
    if stream && messages.len() == 1 {
        let message = messages[0].as_object().ok_or(BridgeError::InvalidShape)?;
        if message.get("role").and_then(Value::as_str) == Some("user")
            && !message
                .get("reasoning_content")
                .is_some_and(|value| !value.is_null() && value != &Value::String(String::new()))
            && let Some(text) = message.get("content").and_then(Value::as_str)
        {
            return Ok(Value::String(text.to_owned()));
        }
    }

    // Convert history in message order while maintaining the function call/result identity ledger.
    let mut input = Vec::new();
    let mut known_calls = BTreeMap::<String, (String, String)>::new();
    let mut item_ids = BTreeSet::new();
    for message in messages {
        let message = message.as_object().ok_or(BridgeError::InvalidShape)?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidShape)?;
        let reasoning = if role == "assistant" {
            chat_reasoning_content(message, reasoning_supported)?
        } else if message
            .get("reasoning_content")
            .is_some_and(|value| !value.is_null() && value != &Value::String(String::new()))
        {
            return Err(BridgeError::UnsupportedSemantics);
        } else {
            None
        };
        match role {
            "assistant" if message.get("tool_calls").is_some() => {
                if message
                    .get("content")
                    .is_some_and(|content| !content.is_null() && content.as_str() != Some(""))
                {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                let calls = message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .ok_or(BridgeError::InvalidShape)?;
                if let Some(reasoning) = reasoning {
                    input.push(reasoning_item(&format!("rs_{}", input.len()), &reasoning));
                }
                for call in calls {
                    let call = call.as_object().ok_or(BridgeError::InvalidShape)?;
                    let id = required_string(call, "id")?;
                    if call.get("type").and_then(Value::as_str) != Some("function") {
                        return Err(BridgeError::UnsupportedSemantics);
                    }
                    let function = call
                        .get("function")
                        .and_then(Value::as_object)
                        .ok_or(BridgeError::InvalidShape)?;
                    let name = required_string(function, "name")?;
                    let arguments = required_string(function, "arguments")?;
                    validate_arguments(&arguments)?;
                    if known_calls
                        .insert(id.clone(), (name.clone(), arguments.clone()))
                        .is_some()
                    {
                        return Err(BridgeError::InvalidToolIdentity);
                    }
                    let item_id =
                        allocate_non_stream_item_id(&id, known_calls.len(), &mut item_ids);
                    input.push(json!({
                        "arguments": arguments,
                        "call_id": id,
                        "id": item_id,
                        "name": name,
                        "type": "function_call"
                    }));
                }
            }
            "tool" => {
                let call_id = required_string(message, "tool_call_id")?;
                if !known_calls.contains_key(&call_id) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let output = message
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or(BridgeError::InvalidShape)?;
                input.push(json!({
                    "call_id": call_id,
                    "output": output,
                    "type": "function_call_output"
                }));
            }
            "user" | "assistant" | "system" | "developer" => {
                let content = message.get("content").ok_or(BridgeError::InvalidShape)?;
                if let Some(reasoning) = reasoning {
                    input.push(reasoning_item(&format!("rs_{}", input.len()), &reasoning));
                }
                let converted = chat_content_to_responses(content, tools_present)?;
                input.push(json!({"content": converted, "role": role, "type": "message"}));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }
    Ok(Value::Array(input))
}

/// Parses the standard Chat `reasoning_effort` field.
fn chat_reasoning_effort(source: &Map<String, Value>) -> Result<Option<String>, BridgeError> {
    // Read only the standard Chat field and reject non-string values.
    source
        .get("reasoning_effort")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(BridgeError::InvalidShape)
        })
        .transpose()
}

/// Reads the assistant message's Provider reasoning extension and converts it to a plain-text reasoning item.
fn chat_reasoning_content(
    message: &Map<String, Value>,
    reasoning_supported: bool,
) -> Result<Option<String>, BridgeError> {
    let Some(value) = message
        .get("reasoning_content")
        .filter(|value| !value.is_null() && *value != &Value::String(String::new()))
    else {
        return Ok(None);
    };
    let text = value.as_str().ok_or(BridgeError::InvalidShape)?;
    if !reasoning_supported {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

/// Builds a plain-text reasoning item that can be replayed by a later Responses turn.
fn reasoning_item(item_id: &str, text: &str) -> Value {
    json!({
        "content": [{"text": text, "type": "reasoning_text"}],
        "id": item_id,
        "status": "completed",
        "summary": [],
        "type": "reasoning"
    })
}

/// Converts Chat message content into Responses input content.
fn chat_content_to_responses(content: &Value, preserve_string: bool) -> Result<Value, BridgeError> {
    // Preserve the permitted plain-text shorthand first so the wire shape of minimal requests remains unchanged.
    match content {
        Value::String(text) if preserve_string => Ok(Value::String(text.clone())),
        Value::String(text) => Ok(json!([{"text": text, "type": "input_text"}])),
        Value::Array(parts) => {
            // Validate and convert each part into a Responses input_text content part.
            let converted = parts
                .iter()
                .map(|part| {
                    let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
                    if part.get("type").and_then(Value::as_str) != Some("text") {
                        return Err(BridgeError::UnsupportedSemantics);
                    }
                    Ok(json!({
                        "text": required_string(part, "text")?,
                        "type": "input_text"
                    }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(converted))
        }
        _ => Err(BridgeError::UnsupportedSemantics),
    }
}

/// Flattens a Chat function-tool schema into a Responses function tool.
fn chat_tool_to_responses(tool: &Value) -> Result<Value, BridgeError> {
    // Validate the Chat tool's function wrapper and copy only modeled function fields.
    let tool = tool.as_object().ok_or(BridgeError::InvalidShape)?;
    let function = tool
        .get("function")
        .and_then(Value::as_object)
        .ok_or(BridgeError::InvalidShape)?;
    // Add the flat function type marker required by Responses.
    let mut result = function.clone();
    result.insert("type".to_owned(), Value::String("function".to_owned()));
    Ok(Value::Object(result))
}

/// Converts a Chat tool choice into a Responses tool choice.
fn chat_tool_choice_to_responses(choice: &Value) -> Result<Value, BridgeError> {
    // Preserve the three string selections shared by both protocols.
    if choice
        .as_str()
        .is_some_and(|choice| matches!(choice, "auto" | "none" | "required"))
    {
        return Ok(choice.clone());
    }
    let choice = choice
        .as_object()
        .ok_or(BridgeError::UnsupportedSemantics)?;
    // Validate the named function selection and flatten the Chat function wrapper.
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(BridgeError::UnsupportedSemantics);
    }
    let function = choice
        .get("function")
        .and_then(Value::as_object)
        .ok_or(BridgeError::UnsupportedSemantics)?;
    Ok(json!({"name": required_string(function, "name")?, "type": "function"}))
}
