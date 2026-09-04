//! Startup credential collection and validation.
//!
//! The builder accepts private configuration material, rejects duplicate or mismatched entries,
//! and drops disabled downstream secrets before creating the immutable runtime snapshot.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

use crate::{
    credential::types::{CredentialId, CredentialMetadata, CredentialType},
    provider::ProviderKind,
};

use super::{
    error::CredentialStoreError,
    material::{CredentialEntry, CredentialMaterial},
    runtime::CredentialStore,
};

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

    /// Adds one Grok OAuth pool member after validating its subscription identity context.
    ///
    /// The subject proves the bundle carries a complete Grok account context, but only the access
    /// token crosses into request-time material because the CLI proxy wire consumes nothing else.
    pub fn insert_grok_oauth_member(
        &mut self,
        pool_id: impl Into<String>,
        member_id: impl Into<String>,
        access_token: SecretString,
        subject: &SecretString,
        metadata: CredentialMetadata,
    ) -> Result<(), CredentialStoreError> {
        // Require the complete subscription-bound OAuth bundle before adding it to the store.
        if subject.expose_secret().trim().is_empty() || metadata.expires_at.is_none() {
            return Err(CredentialStoreError::InvalidOAuthContext);
        }

        // Bind the access token to the sole Provider allowed to consume Grok OAuth material.
        self.insert_upstream_material(
            ProviderKind::Grok,
            pool_id.into(),
            member_id.into(),
            CredentialMaterial::GrokOAuth { access_token },
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
        CredentialStore::from_entries(self.entries)
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
