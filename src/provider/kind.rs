//! Compile-time Provider kinds and static capability contracts.

use crate::{
    core::ApiCapabilities,
    providers::{
        bailian, chatgpt, deepseek, grok, kimi_cn, longcat, mimo, nvidia, openai, openrouter,
        zhipu_cn,
    },
};

use super::ProviderDefinition;

/// Closed set of Providers that Route configuration may reference.
///
/// A new Provider must add an enum variant and its adapter/tests. Unknown strings fail during
/// configuration loading and cannot degrade into a generic HTTP Provider. This keeps authentication
/// and protocol behavior within auditable, compile-time boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    /// ChatGPT subscription access through the fixed Codex backend profile.
    ChatGpt,
    /// Grok subscription access through the fixed CLI proxy backend profile.
    Grok,
    /// OpenAI-compatible provider。
    OpenAi,
    /// LongCat OpenAI-compatible provider。
    LongCat,
    /// DeepSeek OpenAI-compatible provider。
    DeepSeek,
    /// Xiaomi MiMo OpenAI-compatible provider。
    MiMo,
    /// OpenRouter OpenAI-compatible provider。
    OpenRouter,
    /// NVIDIA API Catalog OpenAI-compatible Provider.
    Nvidia,
    /// Alibaba Cloud Model Studio OpenAI-compatible Provider.
    Bailian,
    /// Moonshot Kimi China OpenAI-compatible Provider.
    KimiCn,
    /// Zhipu AI China OpenAI-compatible Provider.
    ZhipuCn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Credential types supported by a Provider.
pub enum CredentialKind {
    /// Uses an HTTP Bearer API key.
    ApiKey,
    /// Uses an externally issued OAuth2 Bearer access token.
    ///
    /// This type describes a credential used for resource requests; it does not mean the gateway
    /// implements token acquisition or refresh.
    OAuth2BearerAccessToken,
}

/// Static Provider capabilities and configuration scope.
///
/// Upstream API capabilities may only narrow this contract and cannot declare adapter features that
/// do not exist. Credential kinds are restricted here as well, preventing Route configuration from
/// becoming a dynamic Provider DSL.
#[derive(Clone, Copy, Debug)]
pub struct ProviderContract {
    kind: ProviderKind,
    capabilities: ApiCapabilities,
    credential_kinds: &'static [CredentialKind],
}

impl ProviderContract {
    /// Creates the static contract for a Provider.
    pub(crate) const fn new(
        kind: ProviderKind,
        capabilities: ApiCapabilities,
        credential_kinds: &'static [CredentialKind],
    ) -> Self {
        Self {
            kind,
            capabilities,
            credential_kinds,
        }
    }

    /// Returns the Provider kind represented by the contract.
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// Returns the capability ceiling supported by the adapter.
    pub fn capabilities(&self) -> &ApiCapabilities {
        &self.capabilities
    }

    /// Returns permitted credential types.
    pub fn credential_kinds(&self) -> &'static [CredentialKind] {
        self.credential_kinds
    }
}

impl ProviderKind {
    /// Returns the stable Provider segment used by a provider/model routing identity.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::Grok => "grok",
            Self::OpenAi => "openai",
            Self::LongCat => "longcat",
            Self::DeepSeek => "deepseek",
            Self::MiMo => "mimo",
            Self::OpenRouter => "openrouter",
            Self::Nvidia => "nvidia",
            Self::Bailian => "bailian",
            Self::KimiCn => "kimi-cn",
            Self::ZhipuCn => "zhipu-cn",
        }
    }

    /// Derives a Provider routing identity from a canonical designer/model identity.
    pub fn routing_model_id(self, canonical_model: &str) -> String {
        let model = canonical_model
            .rsplit_once('/')
            .map_or(canonical_model, |(_, model)| model);
        format!("{}/{model}", self.slug())
    }

    /// Returns the unique compile-time descriptor for the Provider.
    pub fn definition(self) -> &'static ProviderDefinition {
        match self {
            Self::ChatGpt => &chatgpt::DEFINITION,
            Self::Grok => &grok::DEFINITION,
            Self::OpenAi => &openai::DEFINITION,
            Self::LongCat => &longcat::DEFINITION,
            Self::DeepSeek => &deepseek::DEFINITION,
            Self::MiMo => &mimo::DEFINITION,
            Self::OpenRouter => &openrouter::DEFINITION,
            Self::Nvidia => &nvidia::DEFINITION,
            Self::Bailian => &bailian::DEFINITION,
            Self::KimiCn => &kimi_cn::DEFINITION,
            Self::ZhipuCn => &zhipu_cn::DEFINITION,
        }
    }

    /// Returns the Provider's compile-time contract.
    pub fn contract(self) -> &'static ProviderContract {
        self.definition().contract()
    }

    /// Returns a copy of the Provider contract's capability ceiling.
    pub(crate) fn capabilities(self) -> ApiCapabilities {
        *self.contract().capabilities()
    }

    /// Returns whether the static Provider permits the credential kind.
    pub(crate) fn accepts_credential_kind(self, credential: CredentialKind) -> bool {
        self.contract().credential_kinds().contains(&credential)
    }
}
