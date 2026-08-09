//! Bridge request top-level allowlist and function-tool type validation.

use serde_json::{Map, Value};

use crate::core::{ApiProtocol, GenerationRequestField};

use super::super::BridgeError;

/// Validates that request fields and tool types are explicitly supported by the Bridge.
pub(in crate::bridge::conversion) fn reject_unsupported_request(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), BridgeError> {
    // Reject fields outside the typed source catalog or current Bridge representability matrix.
    if source.iter().any(|(wire_name, value)| {
        GenerationRequestField::from_wire(protocol, wire_name).is_none_or(|field| {
            !field.bridge_representable(protocol) && !field.bridge_inactive(value)
        })
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
