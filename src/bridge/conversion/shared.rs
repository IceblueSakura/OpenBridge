//! 双向转换共用的 JSON 校验、字段复制和稳定 identity 映射辅助函数。
//!
//! 本模块不决定协议方向，只提供 request、response 与 stream renderer 共享的纯函数。

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::BridgeError;

/// 解析完整 JSON body，并要求根值为 object。
pub(super) fn parse_value_object(body: &[u8]) -> Result<Map<String, Value>, BridgeError> {
    serde_json::from_slice::<Value>(body)
        .map_err(|_| BridgeError::InvalidShape)?
        .as_object()
        .cloned()
        .ok_or(BridgeError::InvalidShape)
}

/// 从源对象复制显式列出的共同字段。
pub(super) fn copy_fields(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    fields: &[&str],
) {
    // 只复制调用方显式允许的共同字段，不把未知协议字段带入另一方向。
    for field in fields {
        if let Some(value) = source.get(*field) {
            target.insert((*field).to_owned(), value.clone());
        }
    }
}

/// 读取必需的非空字符串字段。
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

/// 验证 function arguments 是非空且闭合的 JSON object。
pub(super) fn validate_arguments(arguments: &str) -> Result<(), BridgeError> {
    if arguments.is_empty()
        || !serde_json::from_str::<Value>(arguments).is_ok_and(|value| value.is_object())
    {
        return Err(BridgeError::InvalidToolArguments);
    }
    Ok(())
}

/// 从 Chat call id 派生 Responses stream item id。
pub(super) fn bridge_item_id(call_id: &str) -> String {
    call_id
        .strip_prefix("call_")
        .map(|suffix| format!("fc_{suffix}"))
        .unwrap_or_else(|| format!("fc_{call_id}"))
}

/// 从 call id 派生非流式 Responses item id 的首选形式。
fn non_stream_item_id(call_id: &str) -> String {
    call_id
        .rsplit_once('_')
        .map(|(_, suffix)| format!("fc_tool_{suffix}"))
        .unwrap_or_else(|| format!("fc_tool_{call_id}"))
}

/// 分配在当前 response 内唯一的非流式 Responses item id。
pub(super) fn allocate_non_stream_item_id(
    call_id: &str,
    ordinal: usize,
    used: &mut BTreeSet<String>,
) -> String {
    // 优先使用由 call id 推导的稳定形式。
    let preferred = non_stream_item_id(call_id);
    if used.insert(preferred.clone()) {
        return preferred;
    }
    // 发生推导冲突时加入序号，确保当前 response 内 identity 唯一。
    let unique = format!("fc_tool_{ordinal}_{}", id_suffix(call_id, "call_"));
    used.insert(unique.clone());
    unique
}

/// 去除已知协议 identity 前缀；前缀不匹配时保留原值。
pub(super) fn id_suffix<'a>(id: &'a str, prefix: &str) -> &'a str {
    id.strip_prefix(prefix).unwrap_or(id)
}

/// 在 Chat 与 Responses identity 前缀之间执行稳定映射。
pub(super) fn map_id(id: &str, from: &str, to: &str) -> String {
    format!("{to}{}", id_suffix(id, from))
}
