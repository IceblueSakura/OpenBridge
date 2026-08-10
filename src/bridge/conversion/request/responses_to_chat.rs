//! Restricted conversion from Responses requests to Chat Completions requests.
//!
//! This module expands input items, wraps function schemas and tool choices, and maintains a
//! function call/output ledger in wire order. Duplicate, unknown, and drifting identities are rejected.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::super::{
    BridgeError,
    shared::{copy_fields, required_string, validate_arguments},
};
use super::structured::responses_text_to_chat;

/// Converts a Responses request that passed top-level validation into a Chat request.
pub(in crate::bridge::conversion) fn responses_request_to_chat(
    source: &Map<String, Value>,
    upstream_model: &str,
    reasoning_supported: bool,
) -> Result<Value, BridgeError> {
    // Expand Responses input into Chat messages and validate the call/output ledger.
    let input = source.get("input").ok_or(BridgeError::InvalidShape)?;
    let messages = responses_input_to_chat(input, reasoning_supported)?;
    let stream = source.get("stream").and_then(Value::as_bool) == Some(true);

    // Copy shared fields and wrap the flat function schema as a Chat function object.
    let mut result = Map::new();
    result.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    result.insert("messages".to_owned(), Value::Array(messages));
    result.insert("stream".to_owned(), Value::Bool(stream));
    copy_fields(
        source,
        &mut result,
        &[
            "parallel_tool_calls",
            "prompt_cache_key",
            "service_tier",
            "temperature",
            "top_p",
        ],
    );
    if let Some(effort) = responses_reasoning_effort(source)? {
        if !reasoning_supported && effort != "none" {
            return Err(BridgeError::UnsupportedSemantics);
        }
        result.insert("reasoning_effort".to_owned(), Value::String(effort));
    }
    if let Some(max_tokens) = source.get("max_output_tokens") {
        result.insert("max_completion_tokens".to_owned(), max_tokens.clone());
    }
    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        result.insert(
            "tools".to_owned(),
            Value::Array(
                tools
                    .iter()
                    .map(responses_tool_to_chat)
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(tool_choice) = source.get("tool_choice") {
        result.insert(
            "tool_choice".to_owned(),
            responses_tool_choice_to_chat(tool_choice)?,
        );
    }
    if let Some(text) = source.get("text").filter(|value| !value.is_null()) {
        result.insert("response_format".to_owned(), responses_text_to_chat(text)?);
    }
    Ok(Value::Object(result))
}

/// Expands Responses input into ordered Chat messages and validates call/result identities.
fn responses_input_to_chat(
    input: &Value,
    reasoning_supported: bool,
) -> Result<Vec<Value>, BridgeError> {
    if let Some(text) = input.as_str() {
        return Ok(vec![json!({"content": text, "role": "user"})]);
    }
    let items = input.as_array().ok_or(BridgeError::InvalidShape)?;
    let mut messages = Vec::new();
    let mut calls = BTreeMap::<String, Value>::new();
    let mut call_order = Vec::new();
    let mut emitted_calls = false;
    let mut seen_results = BTreeSet::new();
    let mut item_ids = BTreeSet::new();
    let mut pending_reasoning = String::new();

    // Build the call ledger in wire order before converting messages and outputs in their original order.
    for item in items {
        let item = item.as_object().ok_or(BridgeError::InvalidShape)?;
        let item_type = item.get("type").and_then(Value::as_str);
        let message_shorthand = item.get("type").is_none()
            && item.len() == 2
            && item.contains_key("role")
            && item.contains_key("content");
        if item_type == Some("message") || message_shorthand {
            let role = required_string(item, "role")?;
            if !pending_reasoning.is_empty() && role != "assistant" {
                return Err(BridgeError::UnsupportedSemantics);
            }
            if !calls.is_empty() && !emitted_calls {
                messages.push(chat_assistant_tool_message(
                    &call_order,
                    &calls,
                    (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
                ));
                pending_reasoning.clear();
                emitted_calls = true;
            }
            messages.push(responses_message_to_chat(
                item,
                (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
            )?);
            pending_reasoning.clear();
            continue;
        }

        match item_type {
            Some("reasoning") => {
                if !reasoning_supported {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                if emitted_calls {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                pending_reasoning.push_str(&responses_reasoning_item_text(item)?);
            }
            Some("function_call") => {
                if emitted_calls {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let call_id = required_string(item, "call_id")?;
                let item_id = required_string(item, "id")?;
                if !item_ids.insert(item_id) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let name = required_string(item, "name")?;
                let arguments = required_string(item, "arguments")?;
                validate_arguments(&arguments)?;
                if calls
                    .insert(
                        call_id.clone(),
                        json!({
                            "function": {"arguments": arguments, "name": name},
                            "id": call_id,
                            "type": "function"
                        }),
                    )
                    .is_some()
                {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                call_order.push(call_id);
            }
            Some("function_call_output") => {
                if !emitted_calls {
                    if calls.is_empty() {
                        return Err(BridgeError::InvalidToolIdentity);
                    }
                    messages.push(chat_assistant_tool_message(
                        &call_order,
                        &calls,
                        (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
                    ));
                    pending_reasoning.clear();
                    emitted_calls = true;
                }
                let call_id = required_string(item, "call_id")?;
                if !calls.contains_key(&call_id) || !seen_results.insert(call_id.clone()) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let output = item.get("output").ok_or(BridgeError::InvalidShape)?;
                let output = output
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| output.to_string());
                messages.push(json!({
                    "content": output,
                    "role": "tool",
                    "tool_call_id": call_id
                }));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }
    if !calls.is_empty() && !emitted_calls {
        messages.push(chat_assistant_tool_message(
            &call_order,
            &calls,
            (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
        ));
        pending_reasoning.clear();
    }
    if !pending_reasoning.is_empty() {
        messages.push(json!({
            "content": Value::Null,
            "reasoning_content": pending_reasoning,
            "role": "assistant"
        }));
    }
    Ok(messages)
}

/// Builds a Chat assistant tool-call message in the original call order.
fn chat_assistant_tool_message(
    order: &[String],
    calls: &BTreeMap<String, Value>,
    reasoning: Option<&str>,
) -> Value {
    let mut message = json!({
        "content": Value::Null,
        "role": "assistant",
        "tool_calls": order.iter().filter_map(|id| calls.get(id)).cloned().collect::<Vec<_>>()
    });
    if let Some(reasoning) = reasoning {
        message["reasoning_content"] = Value::String(reasoning.to_owned());
    }
    message
}

/// Converts one Responses message item into a Chat message.
fn responses_message_to_chat(
    item: &Map<String, Value>,
    reasoning: Option<&str>,
) -> Result<Value, BridgeError> {
    // Read and preserve the Responses message role.
    let role = required_string(item, "role")?;
    let content = item.get("content").ok_or(BridgeError::InvalidShape)?;
    // Merge strings or input_text parts into the Chat single content string.
    let content = match content {
        Value::String(text) => Value::String(text.clone()),
        Value::Array(parts) => {
            let mut text = String::new();
            for part in parts {
                let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
                if part.get("type").and_then(Value::as_str) != Some("input_text") {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                text.push_str(&required_string(part, "text")?);
            }
            Value::String(text)
        }
        _ => return Err(BridgeError::UnsupportedSemantics),
    };
    let mut message = json!({"content": content, "role": role});
    if let Some(reasoning) = reasoning {
        message["reasoning_content"] = Value::String(reasoning.to_owned());
    }
    Ok(message)
}

/// Parses Responses reasoning configuration and converts it to Chat `reasoning_effort`.
fn responses_reasoning_effort(source: &Map<String, Value>) -> Result<Option<String>, BridgeError> {
    // Read only the standard Responses reasoning object and reject unmodeled child fields.
    let object_effort = source
        .get("reasoning")
        .filter(|value| !value.is_null())
        .map(|value| {
            let object = value.as_object().ok_or(BridgeError::InvalidShape)?;
            if object.keys().any(|key| key != "effort") {
                return Err(BridgeError::UnsupportedSemantics);
            }
            object
                .get("effort")
                .filter(|value| !value.is_null())
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .ok_or(BridgeError::InvalidShape)
                })
                .transpose()
        })
        .transpose()?
        .flatten();
    Ok(object_effort)
}

/// Extracts a plain Responses reasoning item and rejects opaque continuation that Chat cannot represent.
fn responses_reasoning_item_text(item: &Map<String, Value>) -> Result<String, BridgeError> {
    reject_encrypted_reasoning(item)?;
    let mut text = String::new();
    for field in ["content", "summary"] {
        let Some(parts) = item.get(field).and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
            let expected = if field == "content" {
                "reasoning_text"
            } else {
                "summary_text"
            };
            if part.get("type").and_then(Value::as_str) != Some(expected) {
                return Err(BridgeError::UnsupportedSemantics);
            }
            text.push_str(&required_string(part, "text")?);
        }
    }
    Ok(text)
}

/// Rejects an opaque Responses reasoning continuation that cannot be represented as plain Chat text.
fn reject_encrypted_reasoning(item: &Map<String, Value>) -> Result<(), BridgeError> {
    let Some(value) = item
        .get("encrypted_content")
        .filter(|value| !value.is_null())
    else {
        return Ok(());
    };
    let content = value.as_str().ok_or(BridgeError::InvalidShape)?;
    if content.is_empty() {
        Ok(())
    } else {
        Err(BridgeError::UnsupportedSemantics)
    }
}

/// Wraps a Responses function-tool schema as a Chat function tool.
fn responses_tool_to_chat(tool: &Value) -> Result<Value, BridgeError> {
    // Validate the flat Responses function tool and remove the type field unused by the target protocol.
    let tool = tool.as_object().ok_or(BridgeError::InvalidShape)?;
    let mut function = tool.clone();
    function.remove("type");
    Ok(json!({"function": function, "type": "function"}))
}

/// Converts a Responses tool choice into a Chat tool choice.
fn responses_tool_choice_to_chat(choice: &Value) -> Result<Value, BridgeError> {
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
    // Validate the named function selection and wrap it again as a Chat function object.
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok(json!({
        "function": {"name": required_string(choice, "name")?},
        "type": "function"
    }))
}
