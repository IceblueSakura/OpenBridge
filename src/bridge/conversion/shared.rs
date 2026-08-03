//! Shared JSON validation, field copying, and stable identity mapping helpers for bidirectional conversion.
//!
//! This module does not choose a protocol direction; it provides pure functions shared by request,
//! response, and stream renderers.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::BridgeError;

/// Parses a complete JSON body and requires an object root.
pub(super) fn parse_value_object(body: &[u8]) -> Result<Map<String, Value>, BridgeError> {
    serde_json::from_slice::<Value>(body)
        .map_err(|_| BridgeError::InvalidShape)?
        .as_object()
        .cloned()
        .ok_or(BridgeError::InvalidShape)
}

/// Copies explicitly listed shared fields from a source object.
pub(super) fn copy_fields(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    fields: &[&str],
) {
    // Copy only fields explicitly allowed by the caller; do not carry unknown protocol fields across.
    for field in fields {
        if let Some(value) = source.get(*field) {
            target.insert((*field).to_owned(), value.clone());
        }
    }
}

/// Reads a required non-empty string field.
pub(super) fn required_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, BridgeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(BridgeError::InvalidShape)
}

/// Validates that function arguments are a non-empty, closed JSON object.
pub(super) fn validate_arguments(arguments: &str) -> Result<(), BridgeError> {
    if arguments.is_empty()
        || !serde_json::from_str::<Value>(arguments).is_ok_and(|value| value.is_object())
    {
        return Err(BridgeError::InvalidToolArguments);
    }
    Ok(())
}

/// Derives a Responses stream item ID from a Chat call ID.
pub(super) fn bridge_item_id(call_id: &str) -> String {
    call_id
        .strip_prefix("call_")
        .map(|suffix| format!("fc_{suffix}"))
        .unwrap_or_else(|| format!("fc_{call_id}"))
}

/// Derives the preferred non-streaming Responses item ID from a call ID.
fn non_stream_item_id(call_id: &str) -> String {
    call_id
        .rsplit_once('_')
        .map(|(_, suffix)| format!("fc_tool_{suffix}"))
        .unwrap_or_else(|| format!("fc_tool_{call_id}"))
}

/// Allocates a non-streaming Responses item ID unique within the current response.
pub(super) fn allocate_non_stream_item_id(
    call_id: &str,
    ordinal: usize,
    used: &mut BTreeSet<String>,
) -> String {
    // Prefer the stable form derived from the call ID.
    let preferred = non_stream_item_id(call_id);
    if used.insert(preferred.clone()) {
        return preferred;
    }
    // Add a sequence when derivation collides so the identity remains unique within the response.
    let unique = format!("fc_tool_{ordinal}_{}", id_suffix(call_id, "call_"));
    used.insert(unique.clone());
    unique
}

/// Removes a known protocol identity prefix and preserves values with other prefixes.
pub(super) fn id_suffix<'a>(id: &'a str, prefix: &str) -> &'a str {
    id.strip_prefix(prefix).unwrap_or(id)
}

/// Performs stable mapping between Chat and Responses identity prefixes.
pub(super) fn map_id(id: &str, from: &str, to: &str) -> String {
    format!("{to}{}", id_suffix(id, from))
}
