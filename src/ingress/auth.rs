//! 当前 Phase 1 使用的静态下游 Bearer 认证。
//!
//! 这是启动时读取的单一开发/受信网络 credential，不是 Phase 4 的 proxy-issued key 管理
//! 系统；比较使用 constant-time equality，且 `Debug` 永不输出 secret。

use std::fmt;

use http::{HeaderMap, header::AUTHORIZATION};
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

/// 用于受保护下游 API 的单一静态 Bearer credential。
pub struct StaticBearerCredential {
    secret: SecretString,
}

impl StaticBearerCredential {
    /// 创建一个不会在 `Debug` 输出中暴露 secret 的 credential。
    pub fn new(secret: SecretString) -> Self {
        Self { secret }
    }

    /// 使用 constant-time 比较校验请求中的 Bearer token。
    pub fn authenticate(&self, headers: &HeaderMap) -> bool {
        let Some(candidate) = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
        else {
            return false;
        };
        let expected = self.secret.expose_secret().as_bytes();
        !expected.is_empty()
            && candidate.len() == expected.len()
            && bool::from(candidate.as_bytes().ct_eq(expected))
    }
}

impl fmt::Debug for StaticBearerCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticBearerCredential")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}
