//! 编译期 Provider 种类与静态能力契约。

use crate::{
    core::ApiCapabilities,
    providers::{longcat, openai},
};

/// 可由 route 配置引用的闭合 provider 集合。
///
/// 新 provider 必须新增 enum 变体及其 adapter/tests；未知字符串在配置加载时失败，不能
/// 退化为“通用 HTTP provider”。这让认证与协议行为保持可审查、可编译的边界。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    /// OpenAI-compatible provider。
    OpenAi,
    /// LongCat OpenAI-compatible provider。
    LongCat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// provider 支持的 credential 类型。
pub enum CredentialKind {
    /// 使用 HTTP Bearer API key。
    ApiKey,
}

/// provider 的静态能力与可配置范围。
///
/// Upstream API capability 只能收窄此契约，不能自行声明 adapter 未实现的特性；endpoint
/// profile 与 credential kind 同样由这里限制，避免 route TOML 变成动态 provider DSL。
#[derive(Debug)]
pub struct ProviderContract {
    kind: ProviderKind,
    capabilities: ApiCapabilities,
    endpoint_profiles: &'static [&'static str],
    credential_kinds: &'static [CredentialKind],
}

impl ProviderContract {
    /// 创建 provider 的静态契约。
    pub const fn new(
        kind: ProviderKind,
        capabilities: ApiCapabilities,
        endpoint_profiles: &'static [&'static str],
        credential_kinds: &'static [CredentialKind],
    ) -> Self {
        Self {
            kind,
            capabilities,
            endpoint_profiles,
            credential_kinds,
        }
    }

    /// 返回契约对应的 provider kind。
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// 返回 adapter 支持的能力上界。
    pub fn capabilities(&self) -> &ApiCapabilities {
        &self.capabilities
    }

    /// 返回允许配置的 endpoint profile 名称。
    pub fn endpoint_profiles(&self) -> &'static [&'static str] {
        self.endpoint_profiles
    }

    /// 返回允许配置的 credential 类型。
    pub fn credential_kinds(&self) -> &'static [CredentialKind] {
        self.credential_kinds
    }
}

impl ProviderKind {
    /// 返回该 provider 的编译期契约。
    pub fn contract(self) -> &'static ProviderContract {
        match self {
            Self::OpenAi => &openai::CONTRACT,
            Self::LongCat => &longcat::CONTRACT,
        }
    }

    pub(crate) fn capabilities(self) -> ApiCapabilities {
        *self.contract().capabilities()
    }

    pub(crate) fn accepts_endpoint_profile(self, profile: &str) -> bool {
        self.contract().endpoint_profiles().contains(&profile)
    }

    pub(crate) fn accepts_credential_kind(self, credential: CredentialKind) -> bool {
        self.contract().credential_kinds().contains(&credential)
    }
}
