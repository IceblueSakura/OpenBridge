//! 下游 Bearer header 解析。

use http::{HeaderMap, header::AUTHORIZATION};

/// 从 Authorization header 提取非空 Bearer token，不记录或复制 token 内容。
pub(super) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}
