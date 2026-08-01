//! 上游 secret 的短时 credential 与当前 credential source。
//!
//! `CredentialValue` 让 adapter 获取认证 header 所需的最小信息，并通过 `SecretString`、
//! redacted `Debug` 和 crate-private exposure 把明文的可见范围限制在 provider egress。

use std::{env, fmt};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use super::ProviderKind;

/// 一次上游调用期间持有的 credential 视图。
///
/// binding id/version 可用于未来审计或 vault rotation，而 secret 本身不能离开 provider
/// 模块；目前仅 adapter 的认证 header 构造可访问其文本。
pub struct CredentialValue {
    provider: ProviderKind,
    binding_id: String,
    secret_version: String,
    secret: SecretString,
}

impl CredentialValue {
    /// 创建一次上游调用期间使用的 credential value。
    pub fn new(
        provider: ProviderKind,
        binding_id: impl Into<String>,
        secret_version: impl Into<String>,
        secret: SecretString,
    ) -> Self {
        Self {
            provider,
            binding_id: binding_id.into(),
            secret_version: secret_version.into(),
            secret,
        }
    }

    /// 返回 credential 所属 provider。
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// 返回代码注册的 credential binding id。
    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    /// 返回 credential 版本标识，不包含 secret 内容。
    pub fn secret_version(&self) -> &str {
        &self.secret_version
    }

    pub(crate) fn expose_secret(&self) -> &str {
        self.secret.expose_secret()
    }
}

impl fmt::Debug for CredentialValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialValue")
            .field("provider", &self.provider)
            .field("binding_id", &self.binding_id)
            .field("secret_version", &self.secret_version)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum CredentialSourceError {
    #[error("upstream credential is unavailable")]
    Unavailable,
    #[error("static upstream credential does not match the configured binding")]
    BindingMismatch,
}

/// 从环境变量或固定测试值解析上游 credential 的来源。
///
/// `Fixed` 仅用于受控测试或显式注入；生产路径通常使用 [`CredentialSource::Environment`]。
pub enum CredentialSource {
    Environment,
    Fixed {
        locator: String,
        secret: SecretString,
    },
}

impl CredentialSource {
    /// 创建从环境变量读取 secret 的来源。
    pub fn environment() -> Self {
        Self::Environment
    }

    /// 创建带有固定 locator 校验的内存 credential 来源。
    pub fn fixed(locator: impl Into<String>, secret: SecretString) -> Self {
        Self::Fixed {
            locator: locator.into(),
            secret,
        }
    }

    /// 按注册表中的 locator 解析一个短时 credential value。
    pub fn resolve(
        &self,
        provider: ProviderKind,
        binding_id: &str,
        locator: &str,
    ) -> Result<CredentialValue, CredentialSourceError> {
        let (secret, version) = match self {
            Self::Environment => {
                let secret = env::var(locator).map_err(|_| CredentialSourceError::Unavailable)?;
                if secret.is_empty() {
                    return Err(CredentialSourceError::Unavailable);
                }
                (SecretString::from(secret), "environment")
            }
            Self::Fixed {
                locator: expected_locator,
                secret,
            } => {
                if locator != expected_locator {
                    return Err(CredentialSourceError::BindingMismatch);
                }
                (
                    SecretString::from(secret.expose_secret().to_owned()),
                    "fixed",
                )
            }
        };
        Ok(CredentialValue::new(provider, binding_id, version, secret))
    }
}

impl fmt::Debug for CredentialSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment => formatter.write_str("CredentialSource::Environment"),
            Self::Fixed { locator, .. } => formatter
                .debug_struct("CredentialSource::Fixed")
                .field("locator", locator)
                .field("secret", &"[REDACTED]")
                .finish(),
        }
    }
}
