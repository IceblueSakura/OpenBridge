//! Purpose-bound secret material and short-lived Provider views.
//!
//! This module keeps Provider OAuth account context inseparable from its access token. The
//! material itself remains private to the credential store; only validated Provider-bound views
//! cross the crate's egress boundary.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};

use crate::{
    credential::types::{CredentialId, CredentialMetadata},
    provider::{CredentialKind, ProviderKind},
};

pub(super) struct CredentialEntry {
    pub(super) id: CredentialId,
    pub(super) material: CredentialMaterial,
    pub(super) metadata: CredentialMetadata,
    pub(super) enabled: bool,
}

/// Secret material variants that keep Provider-specific OAuth context inseparable from its token.
pub(super) enum CredentialMaterial {
    /// A single API key used by downstream users or API-key Providers.
    Single(SecretString),
    /// ChatGPT access token with the routing context required by the Codex backend.
    ChatGptOAuth {
        access_token: SecretString,
        account_id: SecretString,
        is_fedramp_account: bool,
    },
    /// Grok access token; subscription identity stays in the managed document context because the
    /// CLI proxy wire consumes only the Bearer token.
    GrokOAuth { access_token: SecretString },
}

impl CredentialMaterial {
    /// Returns the primary secret used for equality checks and Provider authentication.
    pub(super) fn primary_secret(&self) -> &SecretString {
        match self {
            Self::Single(secret) => secret,
            Self::ChatGptOAuth { access_token, .. } => access_token,
            Self::GrokOAuth { access_token, .. } => access_token,
        }
    }

    /// Returns whether this material contains the complete context required by the credential kind.
    pub(super) fn matches_kind(&self, kind: CredentialKind) -> bool {
        matches!(
            (self, kind),
            (Self::Single(_), CredentialKind::ApiKey)
                | (
                    Self::ChatGptOAuth { .. } | Self::GrokOAuth { .. },
                    CredentialKind::OAuth2BearerAccessToken
                )
        )
    }
}

/// Short-lived upstream credential view with verified Provider ownership.
#[derive(Clone, Copy)]
pub struct UpstreamCredential<'a> {
    pub(super) provider: ProviderKind,
    pub(super) pool_id: &'a str,
    pub(super) member_id: &'a str,
    pub(super) material: &'a CredentialMaterial,
    pub(super) metadata: &'a CredentialMetadata,
}

impl UpstreamCredential<'_> {
    /// Returns the Provider permitted to consume this secret.
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// Returns the code-registered credential pool ID.
    pub fn pool_id(&self) -> &str {
        self.pool_id
    }

    /// Returns the stable non-sensitive member ID within the pool.
    pub fn member_id(&self) -> &str {
        self.member_id
    }

    /// Returns the non-sensitive runtime metadata bound to this view.
    pub fn metadata(&self) -> &CredentialMetadata {
        self.metadata
    }

    /// Exposes the secret only at a Provider egress boundary that completed purpose validation.
    pub(crate) fn expose_secret(&self) -> &str {
        // Expose the secret only after Provider, pool, and kind validation at the egress boundary.
        self.material.primary_secret().expose_secret()
    }

    /// Exposes the ChatGPT account binding only at the Provider authentication boundary.
    pub(crate) fn expose_chatgpt_account_id(&self) -> Option<&str> {
        match self.material {
            CredentialMaterial::ChatGptOAuth { account_id, .. } => Some(account_id.expose_secret()),
            CredentialMaterial::Single(_) | CredentialMaterial::GrokOAuth { .. } => None,
        }
    }

    /// Returns the account routing flag only for complete ChatGPT OAuth material.
    pub(crate) fn is_fedramp_account(&self) -> Option<bool> {
        match self.material {
            CredentialMaterial::ChatGptOAuth {
                is_fedramp_account, ..
            } => Some(*is_fedramp_account),
            CredentialMaterial::Single(_) | CredentialMaterial::GrokOAuth { .. } => None,
        }
    }
}

impl fmt::Debug for UpstreamCredential<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamCredential")
            .field("provider", &self.provider)
            .field("pool_id", &self.pool_id)
            .field("member_id", &self.member_id)
            .field("metadata", &self.metadata)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}
