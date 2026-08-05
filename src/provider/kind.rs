//! Compile-time Provider kinds and static capability contracts.

use crate::{
    core::ApiCapabilities,
    providers::{chatgpt, deepseek, longcat, mimo, openai, openrouter},
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
#[derive(Debug)]
pub struct ProviderContract {
    kind: ProviderKind,
    capabilities: ApiCapabilities,
    credential_kinds: &'static [CredentialKind],
}

impl ProviderContract {
    /// Creates the static contract for a Provider.
    pub const fn new(
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
    /// Returns the unique compile-time descriptor for the Provider.
    pub fn definition(self) -> &'static ProviderDefinition {
        match self {
            Self::ChatGpt => &chatgpt::DEFINITION,
            Self::OpenAi => &openai::DEFINITION,
            Self::LongCat => &longcat::DEFINITION,
            Self::DeepSeek => &deepseek::DEFINITION,
            Self::MiMo => &mimo::DEFINITION,
            Self::OpenRouter => &openrouter::DEFINITION,
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
