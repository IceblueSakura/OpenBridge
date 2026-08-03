//! Shared SSE JSON encoding functions for bidirectional stream renderers.

use serde_json::Value;

use super::super::BridgeError;

/// Encodes a Responses SSE event with both an event name and JSON data.
pub(super) fn response_event(event: &str, value: Value) -> Result<Vec<u8>, BridgeError> {
    let mut output = format!("event: {event}\n").into_bytes();
    output.extend(sse_data(&value)?);
    Ok(output)
}

/// Encodes a JSON value as a complete SSE data block.
pub(super) fn sse_data(value: &Value) -> Result<Vec<u8>, BridgeError> {
    let data = serde_json::to_vec(value).map_err(|_| BridgeError::InvalidShape)?;
    let mut output = b"data: ".to_vec();
    output.extend(data);
    output.extend_from_slice(b"\n\n");
    Ok(output)
}
