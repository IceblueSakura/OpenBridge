//! In-process round-robin selection and member cooldown for Provider credential pools.
//!
//! State keys contain only registry pool IDs, non-sensitive member IDs, and generations. The module
//! holds no secrets, parses no error bodies, and does not turn one member's 429 into a Provider- or
//! target-wide failure.

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::Instant,
};

use http::HeaderMap;

use crate::credential::UpstreamCredential;

use super::health::{DEFAULT_COOLDOWN, MAX_COOLDOWN, retry_after_delay};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct MemberKey {
    member_id: String,
    generation: u64,
}

#[derive(Debug, Default)]
struct PoolState {
    cursor: usize,
    cooldowns: HashMap<MemberKey, Instant>,
}

/// Pool-selection and cooldown state shared by every GatewayState clone.
#[derive(Debug, Default)]
pub(super) struct CredentialHealth {
    pools: Mutex<HashMap<String, PoolState>>,
}

impl CredentialHealth {
    /// Selects an index for a member that is not cooling down or rejected by this request.
    pub(super) fn select_member(
        &self,
        pool_id: &str,
        members: &[UpstreamCredential<'_>],
        rejected: &HashSet<String>,
        now: Instant,
    ) -> Option<usize> {
        // Remove expired cooldowns and scan one full cycle from the shared cursor.
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = pools.entry(pool_id.to_owned()).or_default();
        state.cooldowns.retain(|_, deadline| *deadline > now);
        for offset in 0..members.len() {
            let index = (state.cursor + offset) % members.len();
            let member = &members[index];
            let key = member_key(member);
            if rejected.contains(member.member_id()) || state.cooldowns.contains_key(&key) {
                continue;
            }

            // Advance the cursor immediately after selection so concurrent requests spread naturally.
            state.cursor = (index + 1) % members.len();
            return Some(index);
        }
        None
    }

    /// Returns whether the pool has a healthy member this request may try without advancing the round-robin cursor.
    pub(super) fn has_available_member(
        &self,
        pool_id: &str,
        members: &[UpstreamCredential<'_>],
        rejected: &HashSet<String>,
        now: Instant,
    ) -> bool {
        // Remove expired entries, then perform a side-effect-free availability check.
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = pools.entry(pool_id.to_owned()).or_default();
        state.cooldowns.retain(|_, deadline| *deadline > now);
        members.iter().any(|member| {
            !rejected.contains(member.member_id())
                && !state.cooldowns.contains_key(&member_key(member))
        })
    }

    /// Records a bounded cross-request cooldown for one member from `Retry-After` after a 429.
    pub(super) fn record_rate_limited(
        &self,
        pool_id: &str,
        member: &UpstreamCredential<'_>,
        headers: &HeaderMap,
        now: Instant,
    ) {
        // Use one default for missing or invalid headers and cap unusually long suggestions.
        let delay = retry_after_delay(headers)
            .unwrap_or(DEFAULT_COOLDOWN)
            .min(MAX_COOLDOWN);
        let deadline = now + delay;
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = pools.entry(pool_id.to_owned()).or_default();
        let key = member_key(member);
        state
            .cooldowns
            .entry(key)
            .and_modify(|current| *current = (*current).max(deadline))
            .or_insert(deadline);
    }

    /// A successful response clears only this member's cooldown and does not affect other pool members.
    pub(super) fn record_success(&self, pool_id: &str, member: &UpstreamCredential<'_>) {
        // Remove the exact member key, including generation, so old state cannot affect a new snapshot.
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(state) = pools.get_mut(pool_id) {
            state.cooldowns.remove(&member_key(member));
        }
    }
}

/// Isolates cooldowns from different startup snapshots with non-sensitive member IDs and generations.
fn member_key(member: &UpstreamCredential<'_>) -> MemberKey {
    MemberKey {
        member_id: member.member_id().to_owned(),
        generation: member.metadata().generation(),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use http::{HeaderMap, HeaderValue};
    use secrecy::SecretString;

    use super::CredentialHealth;
    use crate::{
        credential::{
            CredentialMetadata, CredentialSource, CredentialStore, CredentialStoreBuilder,
        },
        provider::{CredentialKind, ProviderKind},
    };

    fn credential_store(generation: u64) -> CredentialStore {
        let mut builder = CredentialStoreBuilder::new();
        for (index, secret) in ["key-a", "key-b"].into_iter().enumerate() {
            builder
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "openai-primary",
                    format!("openai-primary#{}", index + 1),
                    SecretString::from(secret),
                    CredentialMetadata::upstream(
                        CredentialKind::ApiKey,
                        CredentialSource::Programmatic,
                    )
                    .with_generation(generation),
                )
                .unwrap();
        }
        builder.build()
    }

    #[test]
    fn selector_advances_one_shared_round_robin_cursor() {
        // Consecutive selections advance deterministically on one pool state without reading secrets.
        let store = credential_store(1);
        let members = store
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap();
        let selector = CredentialHealth::default();
        let rejected = HashSet::new();
        let now = std::time::Instant::now();
        assert_eq!(
            selector.select_member("openai-primary", &members, &rejected, now),
            Some(0)
        );
        assert_eq!(
            selector.select_member("openai-primary", &members, &rejected, now),
            Some(1)
        );
    }

    #[test]
    fn cooldown_is_capped_and_generation_changes_isolate_old_state() {
        // An excessive Retry-After cools down a member for at most 30 seconds; healthy peers remain selectable.
        let store = credential_store(1);
        let members = store
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap();
        let selector = CredentialHealth::default();
        let rejected = HashSet::new();
        let now = std::time::Instant::now();
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("120"));
        selector.record_rate_limited("openai-primary", &members[0], &headers, now);
        assert_eq!(
            selector.select_member(
                "openai-primary",
                &members,
                &rejected,
                now + Duration::from_secs(29),
            ),
            Some(1)
        );
        assert_eq!(
            selector.select_member(
                "openai-primary",
                &members,
                &rejected,
                now + Duration::from_secs(31),
            ),
            Some(0)
        );

        // A new generation does not inherit an old cooldown even when it reuses a member ID.
        selector.record_rate_limited("openai-primary", &members[0], &headers, now);
        let next_store = credential_store(2);
        let next_members = next_store
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap();
        let rejected_peer = HashSet::from(["openai-primary#2".to_owned()]);
        assert_eq!(
            selector.select_member("openai-primary", &next_members, &rejected_peer, now),
            Some(0)
        );
        assert!(selector.has_available_member("openai-primary", &next_members, &rejected, now,));

        // An invalid Retry-After uses the one-second default instead of permanently blocking the member.
        let default_selector = CredentialHealth::default();
        let mut invalid_headers = HeaderMap::new();
        invalid_headers.insert("retry-after", HeaderValue::from_static("invalid"));
        default_selector.record_rate_limited("openai-primary", &members[0], &invalid_headers, now);
        let rejected_peer = HashSet::from(["openai-primary#2".to_owned()]);
        assert_eq!(
            default_selector.select_member(
                "openai-primary",
                &members,
                &rejected_peer,
                now + Duration::from_millis(500),
            ),
            None
        );
        assert_eq!(
            default_selector.select_member(
                "openai-primary",
                &members,
                &rejected_peer,
                now + Duration::from_secs(2),
            ),
            Some(0)
        );
    }
}
