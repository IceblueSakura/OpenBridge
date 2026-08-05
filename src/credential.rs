//! Startup-built credential snapshot for downstream users and upstream Providers.
//!
//! This module owns downstream-user and upstream-Provider secrets, but keeps the trust directions
//! isolated through purpose-bound [`CredentialId`] values and purpose-specific accessors. The
//! runtime Store does not read configuration files, expose generic plaintext queries, or reveal
//! secrets through `Debug`, errors, or logs.

use std::{fmt, time::SystemTime};

use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::{
    provider::{CredentialKind, ProviderKind},
    registry::RuntimeRegistry,
};

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
    /// Read once from an administrator-selected Codex auth file for an explicit probe.
    CodexAuthFile,
    /// Read once from an OpenBridge-owned OAuth2 auth file during startup.
    OAuth2AuthJsonFile,
    /// Injected directly by the trusted composition root or a test.
    Programmatic,
}

/// Non-sensitive credential metadata frozen with the secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialMetadata {
    credential_type: CredentialType,
    source: CredentialSource,
    generation: u64,
    expires_at: Option<SystemTime>,
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
    fn downstream_user() -> Self {
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

struct CredentialEntry {
    id: CredentialId,
    material: CredentialMaterial,
    metadata: CredentialMetadata,
    enabled: bool,
}

/// Secret material variants that keep Provider-specific OAuth context inseparable from its token.
enum CredentialMaterial {
    /// A single API key used by downstream users or API-key Providers.
    Single(SecretString),
    /// ChatGPT access token with the routing context required by the Codex backend.
    ChatGptOAuth {
        access_token: SecretString,
        account_id: SecretString,
        is_fedramp_account: bool,
    },
}

impl CredentialMaterial {
    /// Returns the primary secret used for equality checks and Provider authentication.
    fn primary_secret(&self) -> &SecretString {
        match self {
            Self::Single(secret) => secret,
            Self::ChatGptOAuth { access_token, .. } => access_token,
        }
    }

    /// Returns whether this material contains the complete context required by the credential kind.
    fn matches_kind(&self, kind: CredentialKind) -> bool {
        matches!(
            (self, kind),
            (Self::Single(_), CredentialKind::ApiKey)
                | (
                    Self::ChatGptOAuth { .. },
                    CredentialKind::OAuth2BearerAccessToken
                )
        )
    }
}

/// Startup builder that collects and validates credentials.
///
/// The builder accepts secrets from private downstream/upstream configuration or controlled tests.
/// [`Self::build`] retains enabled entries and creates an immutable runtime snapshot.
#[derive(Default)]
pub struct CredentialStoreBuilder {
    entries: Vec<CredentialEntry>,
}

impl CredentialStoreBuilder {
    /// Creates an empty credential builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a downstream-user API key and checks ID and key uniqueness across all enabled states.
    pub fn insert_downstream(
        &mut self,
        user_id: impl Into<String>,
        secret: SecretString,
        enabled: bool,
    ) -> Result<(), CredentialStoreError> {
        // Build the downstream-purpose ID and reject duplicate user bindings.
        let id = CredentialId::DownstreamUser {
            user_id: user_id.into(),
        };
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(CredentialStoreError::DuplicateId);
        }

        // Compare every downstream key so enabled state cannot hide a duplicate credential.
        let candidate = secret.expose_secret().as_bytes();
        if self.entries.iter().any(|entry| {
            matches!(entry.id, CredentialId::DownstreamUser { .. })
                && entry.material.primary_secret().expose_secret().as_bytes() == candidate
        }) {
            return Err(CredentialStoreError::DuplicateDownstreamSecret);
        }

        // Retain enabled state temporarily; the final Store keeps enabled users only.
        self.entries.push(CredentialEntry {
            id,
            material: CredentialMaterial::Single(secret),
            metadata: CredentialMetadata::downstream_user(),
            enabled,
        });
        Ok(())
    }

    /// Adds an upstream credential-pool member already parsed by the caller.
    pub fn insert_upstream_member(
        &mut self,
        provider: ProviderKind,
        pool_id: impl Into<String>,
        member_id: impl Into<String>,
        secret: SecretString,
        metadata: CredentialMetadata,
    ) -> Result<(), CredentialStoreError> {
        self.insert_upstream_material(
            provider,
            pool_id.into(),
            member_id.into(),
            CredentialMaterial::Single(secret),
            metadata,
        )
    }

    /// Adds one ChatGPT OAuth pool member with its account-routing context.
    pub fn insert_chatgpt_oauth_member(
        &mut self,
        pool_id: impl Into<String>,
        member_id: impl Into<String>,
        access_token: SecretString,
        account_id: SecretString,
        is_fedramp_account: bool,
        metadata: CredentialMetadata,
    ) -> Result<(), CredentialStoreError> {
        // Require the complete account-bound OAuth bundle before adding it to the shared store.
        if account_id.expose_secret().trim().is_empty() || metadata.expires_at.is_none() {
            return Err(CredentialStoreError::InvalidOAuthContext);
        }

        // Bind the complete bundle to the sole Provider allowed to consume ChatGPT OAuth material.
        self.insert_upstream_material(
            ProviderKind::ChatGpt,
            pool_id.into(),
            member_id.into(),
            CredentialMaterial::ChatGptOAuth {
                access_token,
                account_id,
                is_fedramp_account,
            },
            metadata,
        )
    }

    /// Validates and stores one purpose-bound upstream credential material variant.
    fn insert_upstream_material(
        &mut self,
        provider: ProviderKind,
        pool_id: String,
        member_id: String,
        material: CredentialMaterial,
        metadata: CredentialMetadata,
    ) -> Result<(), CredentialStoreError> {
        // Validate primary secret availability and exact metadata/material kind agreement.
        if material.primary_secret().expose_secret().is_empty() {
            return Err(CredentialStoreError::Unavailable);
        }
        let CredentialType::Upstream(kind) = metadata.credential_type else {
            return Err(CredentialStoreError::InvalidMetadata);
        };
        if metadata.generation == 0 || !material.matches_kind(kind) {
            return Err(CredentialStoreError::InvalidMetadata);
        }

        // Build the Provider- and pool-bound member ID and reject duplicate identities.
        if pool_id.trim().is_empty() || member_id.trim().is_empty() {
            return Err(CredentialStoreError::InvalidPoolIdentity);
        }
        let id = CredentialId::UpstreamPoolMember {
            pool_id: pool_id.clone(),
            member_id,
            provider,
        };
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(CredentialStoreError::DuplicateId);
        }

        // Compare primary secrets within the same Provider-bound pool using constant time.
        let candidate = material.primary_secret().expose_secret().as_bytes();
        if self.entries.iter().any(|entry| {
            matches!(
                &entry.id,
                CredentialId::UpstreamPoolMember {
                    pool_id: configured_pool,
                    provider: configured_provider,
                    ..
                } if configured_pool == &pool_id && *configured_provider == provider
            ) && {
                let expected = entry.material.primary_secret().expose_secret().as_bytes();
                candidate.len() == expected.len() && bool::from(candidate.ct_eq(expected))
            }
        }) {
            return Err(CredentialStoreError::DuplicateUpstreamSecret);
        }

        // Retain the validated material only inside the immutable store entry.
        self.entries.push(CredentialEntry {
            id,
            material,
            metadata,
            enabled: true,
        });
        Ok(())
    }

    /// Builds an immutable runtime snapshot containing enabled credentials only.
    pub fn build(mut self) -> CredentialStore {
        // Drop disabled-user secrets so they do not enter the long-lived runtime Store.
        self.entries.retain(|entry| entry.enabled);
        // Wrap validated entries as the sole runtime owners of the secrets.
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

/// Immutable downstream/upstream credential snapshot for the process lifetime.
pub struct CredentialStore {
    entries: Vec<CredentialEntry>,
}

impl CredentialStore {
    /// Validates runtime pool membership against the registry's state-affinity constraints.
    pub fn validate_registry(
        &self,
        registry: &RuntimeRegistry,
    ) -> Result<(), CredentialStoreError> {
        // Continuations cannot be safely replayed across keys, so the corresponding pool must have exactly one member.
        for pool_id in registry.credential_pool_ids() {
            if !registry.credential_pool_requires_single_member(pool_id) {
                continue;
            }
            let member_count = self
                .entries
                .iter()
                .filter(|entry| {
                    matches!(
                        &entry.id,
                        CredentialId::UpstreamPoolMember {
                            pool_id: configured_pool,
                            ..
                        } if configured_pool == pool_id
                    )
                })
                .count();
            if member_count > 1 {
                return Err(CredentialStoreError::StatefulPoolHasMultipleMembers);
            }
        }
        Ok(())
    }

    /// Matches an enabled downstream API key with constant-time equality and returns its user ID.
    pub fn authenticate_downstream(&self, candidate: &str) -> Option<&str> {
        // Scan every downstream key so an early return cannot expose the match position.
        let candidate = candidate.as_bytes();
        let mut matched = None;
        for entry in &self.entries {
            let CredentialId::DownstreamUser { user_id } = &entry.id else {
                continue;
            };
            let expected = entry.material.primary_secret().expose_secret().as_bytes();
            if candidate.len() == expected.len() && bool::from(candidate.ct_eq(expected)) {
                matched = Some(user_id.as_str());
            }
        }
        matched
    }

    /// Borrows all ordered members for a Provider and pool ID; fail closed on mismatch or emptiness.
    pub fn upstream_pool(
        &self,
        provider: ProviderKind,
        pool_id: &str,
        kind: CredentialKind,
    ) -> Result<Vec<UpstreamCredential<'_>>, CredentialStoreError> {
        // Select only members matching the Provider, pool, and credential kind exactly.
        let members = self
            .entries
            .iter()
            .filter_map(|entry| {
                let CredentialId::UpstreamPoolMember {
                    pool_id: configured_pool,
                    member_id,
                    provider: configured_provider,
                } = &entry.id
                else {
                    return None;
                };
                if configured_pool != pool_id
                    || *configured_provider != provider
                    || entry.metadata.credential_type != CredentialType::Upstream(kind)
                {
                    return None;
                }
                Some(UpstreamCredential {
                    provider,
                    pool_id: configured_pool,
                    member_id,
                    material: &entry.material,
                    metadata: &entry.metadata,
                })
            })
            .collect::<Vec<_>>();
        if members.is_empty() {
            return Err(CredentialStoreError::Unavailable);
        }
        Ok(members)
    }

    /// Enumerates non-sensitive credential IDs for configuration contracts and diagnostic counts.
    pub fn credential_ids(&self) -> impl Iterator<Item = &CredentialId> {
        self.entries.iter().map(|entry| &entry.id)
    }

    /// Enumerates credential IDs and non-sensitive metadata for controlled diagnostics and policy snapshots.
    pub fn credential_metadata(
        &self,
    ) -> impl Iterator<Item = (&CredentialId, &CredentialMetadata)> {
        self.entries
            .iter()
            .map(|entry| (&entry.id, &entry.metadata))
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

/// Short-lived upstream credential view with verified Provider ownership.
pub struct UpstreamCredential<'a> {
    provider: ProviderKind,
    pool_id: &'a str,
    member_id: &'a str,
    material: &'a CredentialMaterial,
    metadata: &'a CredentialMetadata,
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
            CredentialMaterial::Single(_) => None,
        }
    }

    /// Returns the account routing flag only for complete ChatGPT OAuth material.
    pub(crate) fn is_fedramp_account(&self) -> Option<bool> {
        match self.material {
            CredentialMaterial::ChatGptOAuth {
                is_fedramp_account, ..
            } => Some(*is_fedramp_account),
            CredentialMaterial::Single(_) => None,
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

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
/// Credential snapshot construction or purpose-restricted lookup failed.
pub enum CredentialStoreError {
    /// A credential ID is duplicated for the same purpose.
    #[error("credential id is configured more than once")]
    DuplicateId,
    /// The same downstream API key is reused by multiple users.
    #[error("the same downstream API key is configured for more than one user")]
    DuplicateDownstreamSecret,
    /// The same upstream secret is configured more than once in a pool.
    #[error("the same upstream secret is configured more than once in a credential pool")]
    DuplicateUpstreamSecret,
    /// A TargetBound API with continuation enabled references a multi-member pool.
    #[error("state-bound upstream APIs require a single-member credential pool")]
    StatefulPoolHasMultipleMembers,
    /// A pool or member non-sensitive ID is blank.
    #[error("credential pool and member ids must not be blank")]
    InvalidPoolIdentity,
    /// Credential metadata does not match its purpose or has an invalid generation.
    #[error("credential metadata is invalid")]
    InvalidMetadata,
    /// OAuth material is missing its account binding or known expiration.
    #[error("OAuth credential context is invalid")]
    InvalidOAuthContext,
    /// The secret is missing, empty, or does not match the requested purpose/binding.
    #[error("credential is unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::{
        CredentialMetadata, CredentialSource, CredentialStoreBuilder, CredentialStoreError,
    };
    use crate::provider::{CredentialKind, ProviderKind};

    #[test]
    fn state_bound_continuation_rejects_a_multi_member_pool() {
        // Enable continuation for the built-in OpenAI Responses API to create a real state-bound constraint.
        let mut definition = crate::providers::compiled_config();
        if let crate::registry::UpstreamApiCapabilities::Responses(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[1].capabilities
        {
            capabilities.previous_response_id = true;
        }
        let bootstrap =
            crate::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
                .unwrap();
        let registry = crate::registry::build_registry(bootstrap, definition).unwrap();

        // Inject two members and verify startup fails closed instead of guessing key affinity per request.
        let mut credentials = CredentialStoreBuilder::new();
        for (index, secret) in ["key-a", "key-b"].into_iter().enumerate() {
            credentials
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "openai-primary",
                    format!("openai-primary#{}", index + 1),
                    SecretString::from(secret),
                    CredentialMetadata::upstream(
                        CredentialKind::ApiKey,
                        CredentialSource::Programmatic,
                    ),
                )
                .unwrap();
        }
        assert_eq!(
            credentials.build().validate_registry(&registry),
            Err(CredentialStoreError::StatefulPoolHasMultipleMembers)
        );
    }

    #[test]
    fn runtime_store_owns_a_redacted_snapshot_and_rejects_empty_upstream_secrets() {
        // Inject a startup-parsed secret and build the immutable runtime snapshot.
        let mut credentials = CredentialStoreBuilder::new();
        credentials
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "openai-primary",
                "openai-primary#1",
                SecretString::from("startup-secret"),
                CredentialMetadata::upstream(
                    CredentialKind::ApiKey,
                    CredentialSource::UpstreamConfiguration,
                ),
            )
            .unwrap();

        // Verify that the runtime Store retains the startup snapshot and Debug output contains no plaintext.
        let credentials = credentials.build();
        let credential = credentials
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap()
            .remove(0);
        assert_eq!(credential.expose_secret(), "startup-secret");
        assert_eq!(
            credential.metadata().source(),
            CredentialSource::UpstreamConfiguration
        );
        assert_eq!(credential.metadata().generation(), 1);
        assert_eq!(credential.metadata().expires_at(), None);
        assert!(!format!("{credentials:?} {credential:?}").contains("startup-secret"));

        // Reject an empty upstream key so the error occurs outside the request path.
        let mut invalid = CredentialStoreBuilder::new();
        assert_eq!(
            invalid
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "empty",
                    "empty#1",
                    SecretString::from(String::new()),
                    CredentialMetadata::upstream(
                        CredentialKind::ApiKey,
                        CredentialSource::Programmatic,
                    ),
                )
                .unwrap_err(),
            CredentialStoreError::Unavailable
        );
        assert_eq!(
            invalid
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "invalid-generation",
                    "invalid-generation#1",
                    SecretString::from("synthetic"),
                    CredentialMetadata::upstream(
                        CredentialKind::ApiKey,
                        CredentialSource::Programmatic,
                    )
                    .with_generation(0),
                )
                .unwrap_err(),
            CredentialStoreError::InvalidMetadata
        );
        assert_eq!(
            invalid
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    " ",
                    "member",
                    SecretString::from("synthetic"),
                    CredentialMetadata::upstream(
                        CredentialKind::ApiKey,
                        CredentialSource::Programmatic,
                    ),
                )
                .unwrap_err(),
            CredentialStoreError::InvalidPoolIdentity
        );
    }

    #[test]
    fn builder_rejects_duplicate_bindings_and_invalid_upstream_metadata() {
        // Reject duplicate downstream identities and secrets even when the original user is disabled.
        let mut downstream = CredentialStoreBuilder::new();
        downstream
            .insert_downstream("user-a", SecretString::from("downstream-a"), false)
            .unwrap();
        assert_eq!(
            downstream
                .insert_downstream("user-a", SecretString::from("downstream-b"), true)
                .unwrap_err(),
            CredentialStoreError::DuplicateId
        );
        assert_eq!(
            downstream
                .insert_downstream("user-b", SecretString::from("downstream-a"), true)
                .unwrap_err(),
            CredentialStoreError::DuplicateDownstreamSecret
        );

        // Reject duplicate member identities and secrets within the same Provider-bound pool.
        let metadata =
            CredentialMetadata::upstream(CredentialKind::ApiKey, CredentialSource::Programmatic);
        let mut upstream = CredentialStoreBuilder::new();
        upstream
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "pool-a",
                "pool-a#1",
                SecretString::from("upstream-a"),
                metadata,
            )
            .unwrap();
        assert_eq!(
            upstream
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "pool-a",
                    "pool-a#1",
                    SecretString::from("upstream-b"),
                    metadata,
                )
                .unwrap_err(),
            CredentialStoreError::DuplicateId
        );
        assert_eq!(
            upstream
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "pool-a",
                    "pool-a#2",
                    SecretString::from("upstream-a"),
                    metadata,
                )
                .unwrap_err(),
            CredentialStoreError::DuplicateUpstreamSecret
        );

        // Reject blank member IDs and metadata for the downstream credential purpose.
        assert_eq!(
            upstream
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "pool-b",
                    " ",
                    SecretString::from("upstream-b"),
                    metadata,
                )
                .unwrap_err(),
            CredentialStoreError::InvalidPoolIdentity
        );
        assert_eq!(
            upstream
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "pool-b",
                    "pool-b#1",
                    SecretString::from("upstream-b"),
                    CredentialMetadata::downstream_user(),
                )
                .unwrap_err(),
            CredentialStoreError::InvalidMetadata
        );
    }
}
