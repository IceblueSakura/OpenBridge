//! Chat 与 Responses bridge 状态机共用的 wire 字段与 arguments 校验。

use serde_json::Value;

use super::BridgeStreamError;

/// 读取必需字符串字段。
pub(super) fn required_str<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a str, BridgeStreamError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(BridgeStreamError::InvalidJson)
}

/// 读取必需无符号整数 identity 字段。
pub(super) fn required_u64(value: &Value, field: &str) -> Result<u64, BridgeStreamError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(BridgeStreamError::InvalidJson)
}

/// 验证 function arguments 是完整 JSON object。
pub(super) fn validate_arguments(arguments: &str) -> Result<(), BridgeStreamError> {
    // function arguments 必须是完整 JSON object，不能仅凭字符串结束位置推断完成。
    let parsed: Value =
        serde_json::from_str(arguments).map_err(|_| BridgeStreamError::InvalidToolArguments)?;
    if parsed.is_object() {
        Ok(())
    } else {
        Err(BridgeStreamError::InvalidToolArguments)
    }
}
