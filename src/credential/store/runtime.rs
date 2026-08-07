//! Immutable runtime credential snapshot and purpose-restricted lookup.
//!
//! The runtime store owns only enabled, startup-validated entries. It authenticates downstream
//! users and returns Provider-, pool-, and kind-bound views without exposing generic secret lookup.

use std::fmt;

use secrecy::ExposeSecret;
use subtle::ConstantTimeEq;

use crate::{
    credential::types::{CredentialId, CredentialMetadata, CredentialType},
    provider::{CredentialKind, ProviderKind},
    registry::RuntimeRegistry,
};

use super::{
    error::CredentialStoreError,
    material::{CredentialEntry, UpstreamCredential},
};

/// Immutable downstream/upstream credential snapshot for the process lifetime.
pub struct CredentialStore {
    entries: Vec<CredentialEntry>,
}

impl CredentialStore {
    /// Wraps the builder's validated entries as the process-lifetime snapshot.
    pub(super) fn from_entries(entries: Vec<CredentialEntry>) -> Self {
        Self { entries }
    }

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
