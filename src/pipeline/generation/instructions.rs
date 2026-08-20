//! Shared generation-instruction analysis and canonical request normalization.
//!
//! This module owns only protocol envelope policy. It does not inspect Providers, select Routes,
//! or reinterpret transcript messages after the first Chat entry.

use bytes::Bytes;
use serde_json::{Map, Value, json};

use crate::core::ApiProtocol;

use super::super::{error::RequestPlanningError, types::RequestedInstructions};

/// Extracts the sole client-owned instruction source without consulting the registry.
pub(super) fn analyze_requested_instructions(
    protocol: ApiProtocol,
    object: &Map<String, Value>,
) -> Result<RequestedInstructions, RequestPlanningError> {
    match protocol {
        ApiProtocol::Responses => match object.get("instructions") {
            None => Ok(RequestedInstructions::Default),
            Some(Value::String(value)) if !value.trim().is_empty() => {
                Ok(RequestedInstructions::Client(value.clone()))
            }
            Some(_) => Err(RequestPlanningError::InvalidInstructions),
        },
        ApiProtocol::ChatCompletions => analyze_chat_instructions(object),
    }
}

/// Accepts only an explicit false state flag; omission remains the stateless default.
pub(super) fn validate_stateless_store(
    object: &Map<String, Value>,
) -> Result<(), RequestPlanningError> {
    if object
        .get("store")
        .is_some_and(|value| value.as_bool() != Some(false))
    {
        return Err(RequestPlanningError::InvalidStore);
    }
    Ok(())
}

/// Applies one analyzed instruction source before immutable candidate expansion.
pub(super) fn normalize_generation_request(
    body: &Bytes,
    protocol: ApiProtocol,
    requested: &RequestedInstructions,
    default_instructions: &str,
) -> Result<Bytes, RequestPlanningError> {
    let mut document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    if protocol == ApiProtocol::ChatCompletions {
        validate_general_chat_messages(object)?;
    }
    apply_instruction_policy(object, protocol, requested, default_instructions)?;
    serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)
}

/// Applies the same trusted policy to one built-in probe request.
pub(crate) fn normalize_probe_generation_request(
    protocol: ApiProtocol,
    document: &mut Value,
    default_instructions: &str,
) -> Result<(), RequestPlanningError> {
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    let requested = analyze_requested_instructions(protocol, object)?;
    validate_stateless_store(object)?;
    if protocol == ApiProtocol::ChatCompletions {
        validate_general_chat_messages(object)?;
    }
    apply_instruction_policy(object, protocol, &requested, default_instructions)
}

/// Extracts only the first eligible Chat source without imposing general-model rules on specialized tasks.
fn analyze_chat_instructions(
    object: &Map<String, Value>,
) -> Result<RequestedInstructions, RequestPlanningError> {
    let Some(first) = object
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.first())
        .and_then(Value::as_object)
    else {
        return Ok(RequestedInstructions::Default);
    };
    let Some(role) = first.get("role").and_then(Value::as_str) else {
        return Ok(RequestedInstructions::Default);
    };
    if matches!(role, "system" | "developer")
        && let Some(text) = first
            .get("content")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
    {
        return Ok(RequestedInstructions::Client(text.to_owned()));
    }
    Ok(RequestedInstructions::Default)
}

/// Validates the Chat envelope used by the project-wide general Generation policy.
fn validate_general_chat_messages(object: &Map<String, Value>) -> Result<(), RequestPlanningError> {
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .filter(|messages| !messages.is_empty())
        .ok_or(RequestPlanningError::InvalidMessages)?;

    // Validate every role now so Native preservation cannot bypass the same strict protocol edge.
    for message in messages {
        let message = message
            .as_object()
            .ok_or(RequestPlanningError::InvalidMessages)?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(RequestPlanningError::InvalidMessages)?;
        if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
            return Err(RequestPlanningError::InvalidMessages);
        }
        let content = message.get("content");
        let content_is_chat_value =
            content.is_some_and(|value| value.is_string() || value.is_array());
        let valid_shape = match role {
            "system" | "developer" | "user" => content_is_chat_value,
            "assistant" => {
                content
                    .is_some_and(|value| value.is_null() || value.is_string() || value.is_array())
                    || message.get("tool_calls").is_some_and(Value::is_array)
            }
            "tool" => {
                content_is_chat_value
                    && message
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
            }
            _ => false,
        };
        if !valid_shape {
            return Err(RequestPlanningError::InvalidMessages);
        }
    }
    Ok(())
}

/// Encodes one effective value without scanning, joining, or rewriting later transcript messages.
fn apply_instruction_policy(
    object: &mut Map<String, Value>,
    protocol: ApiProtocol,
    requested: &RequestedInstructions,
    default_instructions: &str,
) -> Result<(), RequestPlanningError> {
    let effective = match requested {
        RequestedInstructions::Client(value) => value,
        RequestedInstructions::Default => default_instructions,
    };
    debug_assert!(!effective.trim().is_empty());

    match protocol {
        ApiProtocol::Responses => {
            object.insert(
                "instructions".to_owned(),
                Value::String(effective.to_owned()),
            );
            object.insert("store".to_owned(), Value::Bool(false));
        }
        ApiProtocol::ChatCompletions if matches!(requested, RequestedInstructions::Default) => {
            let messages = object
                .get_mut("messages")
                .and_then(Value::as_array_mut)
                .ok_or(RequestPlanningError::InvalidMessages)?;
            messages.insert(0, json!({"role": "system", "content": effective}));
        }
        ApiProtocol::ChatCompletions => {}
    }
    Ok(())
}
