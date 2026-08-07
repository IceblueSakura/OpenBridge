//! Purpose-bound credential identities and non-sensitive metadata.
//!
//! These types describe why a secret may be used and where it came from without storing file
//! locators or secret values. Material ownership and purpose-restricted access remain in `store`.

use std::time::SystemTime;

use crate::provider::{CredentialKind, ProviderKind};

/// Stable runtime identity and purpose for one credential.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialId {
    /// Downstream-user Bearer API key.
    DownstreamUser {
        /// Stable user ID bound after authentication.
        user_id: String,
    },
    /// Upstream Provider credential-pool member.
    UpstreamPoolMember {
        /// Pool ID unique within the registry.
        pool_id: String,
        /// Stable non-sensitive member ID within the pool.
        member_id: String,
        /// Provider permitted to consume this secret.
        provider: ProviderKind,
    },
}

/// Credential purpose and authentication type within the Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialType {
    /// Downstream-user Bearer API key.
    DownstreamApiKey,
    /// Credential kind declared by the upstream Provider.
    Upstream(CredentialKind),
}

/// Trusted source category for a secret entering the Store.
///
/// This enum retains only low-sensitivity categories; it stores no file paths, issuer URLs, or other source details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialSource {
    /// From private downstream-user configuration.
    UserConfiguration,
    /// From private upstream credential configuration.
    UpstreamConfiguration,
    /// Loaded from an OpenBridge-owned OAuth2 auth file and maintained by its guarded manager.
    OAuth2AuthJsonFile,
    /// Injected directly by the trusted composition root or a test.
    Programmatic,
}

/// Non-sensitive credential metadata frozen with the secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialMetadata {
    pub(super) credential_type: CredentialType,
    pub(super) source: CredentialSource,
    pub(super) generation: u64,
    pub(super) expires_at: Option<SystemTime>,
}

impl CredentialMetadata {
    /// Creates first-generation upstream credential metadata.
    pub fn upstream(kind: CredentialKind, source: CredentialSource) -> Self {
        Self {
            credential_type: CredentialType::Upstream(kind),
            source,
            generation: 1,
            expires_at: None,
        }
    }

    /// Creates fixed metadata for a downstream-user API key.
    pub(super) fn downstream_user() -> Self {
        Self {
            credential_type: CredentialType::DownstreamApiKey,
            source: CredentialSource::UserConfiguration,
            generation: 1,
            expires_at: None,
        }
    }

    /// Overrides credential generation; zero is rejected when inserted into the Store.
    pub fn with_generation(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }

    /// Sets the credential's known expiration time.
    pub fn with_expires_at(mut self, expires_at: SystemTime) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Returns the credential purpose and authentication type.
    pub fn credential_type(&self) -> CredentialType {
        self.credential_type
    }

    /// Returns the trusted source category for the secret.
    pub fn source(&self) -> CredentialSource {
        self.source
    }

    /// Returns the credential generation, which starts at one.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the known expiration time, or `None` when no static source provides one.
    pub fn expires_at(&self) -> Option<SystemTime> {
        self.expires_at
    }
}
