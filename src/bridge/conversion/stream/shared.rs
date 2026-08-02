//! 双向 stream renderer 共用的 SSE JSON 编码函数。

use serde_json::Value;

use super::super::BridgeError;

/// 编码同时携带 event 名称与 JSON data 的 Responses SSE event。
pub(super) fn response_event(event: &str, value: Value) -> Result<Vec<u8>, BridgeError> {
    let mut output = format!("event: {event}\n").into_bytes();
    output.extend(sse_data(&value)?);
    Ok(output)
}

/// 将 JSON 值编码为一个完整的 SSE data block。
pub(super) fn sse_data(value: &Value) -> Result<Vec<u8>, BridgeError> {
    let data = serde_json::to_vec(value).map_err(|_| BridgeError::InvalidShape)?;
    let mut output = b"data: ".to_vec();
    output.extend(data);
    output.extend_from_slice(b"\n\n");
    Ok(output)
}
