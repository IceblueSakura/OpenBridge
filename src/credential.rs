//! 启动时构造的上下游 credential 快照。
//!
//! 本模块统一持有下游用户与上游 Provider 的 secret，但通过带用途的 [`CredentialId`]
//! 和目的专用访问方法保持两个信任方向隔离。运行时 Store 不读取文件或环境变量，不提供
//! 通用明文查询，也不会在 `Debug`、错误或日志中暴露 secret。

use std::{env, fmt};

use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::{provider::ProviderKind, registry::UpstreamTarget};

/// 一项 credential 的稳定运行时标识与用途。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialId {
    /// 下游用户 Bearer API Key。
    DownstreamUser {
        /// 认证成功后绑定的稳定用户 ID。
        user_id: String,
    },
    /// 上游 Provider credential binding。
    UpstreamBinding {
        /// 注册表中全局唯一的 binding ID。
        binding_id: String,
        /// 允许消费此 secret 的 Provider。
        provider: ProviderKind,
    },
}

struct CredentialEntry {
    id: CredentialId,
    secret: SecretString,
    enabled: bool,
}

/// 启动阶段收集并校验 credential 的构造器。
///
/// Builder 可以接收来自私有用户文件、环境变量或受控测试的 secret；调用 [`Self::build`]
/// 后只保留启用项，并生成不可变运行时快照。
#[derive(Default)]
pub struct CredentialStoreBuilder {
    entries: Vec<CredentialEntry>,
}

impl CredentialStoreBuilder {
    /// 创建空的 credential 构造器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 加入一个下游用户 API Key，并在所有启用状态之间检查 ID 与 Key 唯一性。
    pub fn insert_downstream(
        &mut self,
        user_id: impl Into<String>,
        secret: SecretString,
        enabled: bool,
    ) -> Result<(), CredentialStoreError> {
        // 构造带下游用途的 ID，并拒绝重复用户绑定。
        let id = CredentialId::DownstreamUser {
            user_id: user_id.into(),
        };
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(CredentialStoreError::DuplicateId);
        }

        // 比较全部下游 Key，避免启停状态掩盖重复 credential。
        let candidate = secret.expose_secret().as_bytes();
        if self.entries.iter().any(|entry| {
            matches!(entry.id, CredentialId::DownstreamUser { .. })
                && entry.secret.expose_secret().as_bytes() == candidate
        }) {
            return Err(CredentialStoreError::DuplicateDownstreamSecret);
        }

        // 暂存启停状态，构造最终 Store 时只保留启用用户。
        self.entries.push(CredentialEntry {
            id,
            secret,
            enabled,
        });
        Ok(())
    }

    /// 加入一个已由调用方解析的上游 API Key。
    pub fn insert_upstream(
        &mut self,
        provider: ProviderKind,
        binding_id: impl Into<String>,
        secret: SecretString,
    ) -> Result<(), CredentialStoreError> {
        // 拒绝空 secret，避免服务启动后才产生上游认证失败。
        if secret.expose_secret().is_empty() {
            return Err(CredentialStoreError::Unavailable);
        }

        // 构造带 Provider 归属的上游 ID，并拒绝重复 binding。
        let id = CredentialId::UpstreamBinding {
            binding_id: binding_id.into(),
            provider,
        };
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(CredentialStoreError::DuplicateId);
        }
        self.entries.push(CredentialEntry {
            id,
            secret,
            enabled: true,
        });
        Ok(())
    }

    /// 从目标已校验的环境变量 locator 读取一次上游 secret。
    pub fn load_upstream_environment(
        &mut self,
        target: &UpstreamTarget,
    ) -> Result<(), CredentialStoreError> {
        // 只在启动阶段调用进程环境解析器，运行时 Store 不保存解析函数。
        self.load_upstream_with(
            target.kind(),
            target.credential().id(),
            target.credential().secret_reference().locator(),
            |locator| env::var(locator).ok(),
        )
    }

    fn load_upstream_with(
        &mut self,
        provider: ProviderKind,
        binding_id: &str,
        locator: &str,
        resolve: impl FnOnce(&str) -> Option<String>,
    ) -> Result<(), CredentialStoreError> {
        // 读取一次受信 locator，并把缺失、非 Unicode 或空值统一视为不可用。
        let secret = resolve(locator).ok_or(CredentialStoreError::Unavailable)?;
        // 将来源值移入 Store builder，运行时不再保留或访问 locator。
        self.insert_upstream(provider, binding_id, SecretString::from(secret))
    }

    /// 构造只包含启用 credential 的不可变运行时快照。
    pub fn build(mut self) -> CredentialStore {
        // 丢弃禁用用户的 secret，确保它们不进入长期运行时 Store。
        self.entries.retain(|entry| entry.enabled);
        // 将完成校验的条目封装为唯一运行时 secret 所有者。
        CredentialStore {
            entries: self.entries,
        }
    }
}

impl fmt::Debug for CredentialStoreBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialStoreBuilder")
            .field("entries", &self.entries.len())
            .finish()
    }
}

/// 进程生命周期内不可变的上下游 credential 快照。
pub struct CredentialStore {
    entries: Vec<CredentialEntry>,
}

impl CredentialStore {
    /// 使用 constant-time equality 匹配启用的下游 API Key，并返回对应用户 ID。
    pub fn authenticate_downstream(&self, candidate: &str) -> Option<&str> {
        // 遍历全部下游 Key，避免通过提前返回暴露匹配位置。
        let candidate = candidate.as_bytes();
        let mut matched = None;
        for entry in &self.entries {
            let CredentialId::DownstreamUser { user_id } = &entry.id else {
                continue;
            };
            let expected = entry.secret.expose_secret().as_bytes();
            if candidate.len() == expected.len() && bool::from(candidate.ct_eq(expected)) {
                matched = Some(user_id.as_str());
            }
        }
        matched
    }

    /// 按 Provider 与 binding ID 借用上游 credential，归属不匹配时 fail closed。
    pub fn upstream(
        &self,
        provider: ProviderKind,
        binding_id: &str,
    ) -> Result<UpstreamCredential<'_>, CredentialStoreError> {
        // 只匹配完整上游用途 ID，不能把下游 Key 或其他 Provider 的 Key 作为候选。
        let entry = self.entries.iter().find(|entry| {
            matches!(
                &entry.id,
                CredentialId::UpstreamBinding {
                    binding_id: configured_binding,
                    provider: configured_provider,
                } if configured_binding == binding_id && *configured_provider == provider
            )
        });
        let Some(entry) = entry else {
            return Err(CredentialStoreError::Unavailable);
        };

        // 从 Store 条目借用稳定 binding 元数据与 secret。
        let CredentialId::UpstreamBinding {
            binding_id,
            provider,
        } = &entry.id
        else {
            return Err(CredentialStoreError::Unavailable);
        };
        // 返回只在认证 header 构造边界可暴露明文的借用视图。
        Ok(UpstreamCredential {
            provider: *provider,
            binding_id,
            secret: &entry.secret,
        })
    }

    /// 枚举非敏感 credential ID，供配置契约和诊断计数验证。
    pub fn credential_ids(&self) -> impl Iterator<Item = &CredentialId> {
        self.entries.iter().map(|entry| &entry.id)
    }
}

impl fmt::Debug for CredentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let downstream = self
            .entries
            .iter()
            .filter(|entry| matches!(entry.id, CredentialId::DownstreamUser { .. }))
            .count();
        let upstream = self.entries.len() - downstream;
        formatter
            .debug_struct("CredentialStore")
            .field("downstream_credentials", &downstream)
            .field("upstream_credentials", &upstream)
            .finish()
    }
}

/// 已验证 Provider 归属的短时上游 credential 借用视图。
pub struct UpstreamCredential<'a> {
    provider: ProviderKind,
    binding_id: &'a str,
    secret: &'a SecretString,
}

impl UpstreamCredential<'_> {
    /// 返回允许消费此 secret 的 Provider。
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// 返回代码注册的 credential binding ID。
    pub fn binding_id(&self) -> &str {
        self.binding_id
    }

    pub(crate) fn expose_secret(&self) -> &str {
        self.secret.expose_secret()
    }
}

impl fmt::Debug for UpstreamCredential<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamCredential")
            .field("provider", &self.provider)
            .field("binding_id", &self.binding_id)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
/// credential 快照构造或用途受限查询失败。
pub enum CredentialStoreError {
    /// 相同用途的 credential ID 重复。
    #[error("credential id is configured more than once")]
    DuplicateId,
    /// 同一个下游 API Key 被多个用户复用。
    #[error("the same downstream API key is configured for more than one user")]
    DuplicateDownstreamSecret,
    /// secret 缺失、为空或与请求的用途/binding 不匹配。
    #[error("credential is unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::{CredentialStoreBuilder, CredentialStoreError};
    use crate::provider::ProviderKind;

    #[test]
    fn runtime_store_owns_a_redacted_snapshot_and_rejects_empty_upstream_secrets() {
        // 模拟启动来源返回一份可随后变化的字符串，并确认只解析一次。
        let mut source = "startup-secret".to_owned();
        let mut credentials = CredentialStoreBuilder::new();
        credentials
            .load_upstream_with(
                ProviderKind::OpenAi,
                "openai-primary",
                "SYNTHETIC_API_KEY",
                |_| Some(source.clone()),
            )
            .unwrap();
        source.clear();

        // 验证运行时 Store 保留启动快照，且任何 Debug 输出都不包含明文。
        let credentials = credentials.build();
        let credential = credentials
            .upstream(ProviderKind::OpenAi, "openai-primary")
            .unwrap();
        assert_eq!(credential.expose_secret(), "startup-secret");
        assert!(!format!("{credentials:?} {credential:?}").contains("startup-secret"));

        // 拒绝缺失或空的上游 Key，确保错误发生在请求路径之外。
        let mut invalid = CredentialStoreBuilder::new();
        assert_eq!(
            invalid
                .load_upstream_with(ProviderKind::OpenAi, "missing", "MISSING_API_KEY", |_| None,)
                .unwrap_err(),
            CredentialStoreError::Unavailable
        );
        assert_eq!(
            invalid
                .insert_upstream(
                    ProviderKind::OpenAi,
                    "empty",
                    SecretString::from(String::new()),
                )
                .unwrap_err(),
            CredentialStoreError::Unavailable
        );
    }
}
