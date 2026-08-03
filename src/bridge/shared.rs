//! Shared wire-field and argument validation for the Chat and Responses Bridge state machines.

use serde_json::Value;

use super::BridgeStreamError;

/// Reads a required string field.
pub(super) fn required_str<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a str, BridgeStreamError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(BridgeStreamError::InvalidJson)
}

/// Reads a required unsigned-integer identity field.
pub(super) fn required_u64(value: &Value, field: &str) -> Result<u64, BridgeStreamError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(BridgeStreamError::InvalidJson)
}

/// Validates that function arguments are a complete JSON object.
pub(super) fn validate_arguments(arguments: &str) -> Result<(), BridgeStreamError> {
    // Function arguments must be a complete JSON object; a string boundary alone cannot prove completion.
    let parsed: Value =
        serde_json::from_str(arguments).map_err(|_| BridgeStreamError::InvalidToolArguments)?;
    if parsed.is_object() {
        Ok(())
    } else {
        Err(BridgeStreamError::InvalidToolArguments)
    }
}
