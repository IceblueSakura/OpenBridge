//! In-memory OAuth2 credential aggregate, request lease, and lifecycle state.

use std::{
    fmt,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use secrecy::{ExposeSecret, SecretString};
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    credential::{
        CredentialMetadata, CredentialSource, CredentialStore, CredentialStoreBuilder,
        CredentialStoreError, UpstreamCredential,
    },
    provider::{CredentialKind, ProviderKind},
};

use super::super::{
    document::ValidatedOAuth2Bundle,
    storage::{OAuth2AuthFileVersion, OAuth2LoginTarget},
    transport::refresh::RefreshTransportError,
};

pub(super) const REFRESH_SAFETY_WINDOW: Duration = Duration::from_secs(120);
const MAX_EARLY_JITTER: u64 = 30;
const INITIAL_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuth2Credential {
    provider: ProviderKind,
    pool_id: String,
    member_id: String,
    metadata: CredentialMetadata,
    status: OAuth2CredentialStatus,
}

impl OAuth2Credential {
    /// Returns the Provider permitted to consume this credential.
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// Returns the compile-time credential binding ID.
    pub fn pool_id(&self) -> &str {
        &self.pool_id
    }

    /// Returns the sole stable member ID derived from the binding ID.
    pub fn member_id(&self) -> &str {
        &self.member_id
    }

    /// Returns non-sensitive metadata from this atomic lifecycle snapshot.
    pub fn metadata(&self) -> &CredentialMetadata {
        &self.metadata
    }

    /// Returns the coarse lifecycle state without exposing tokens or account identity.
    pub fn status(&self) -> OAuth2CredentialStatus {
        self.status
    }
}

/// Coarse, value-free lifecycle status of one managed OAuth2 credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuth2CredentialStatus {
    /// The persisted token is active or waiting for its expiry-derived due time.
    Active,
    /// A confirmed transient failure is waiting for bounded retry.
    RefreshBackoff,
    /// The authority rejected the refresh credential and explicit login is required.
    ReauthRequired,
    /// Rotation may have occurred without a safely persisted result; automatic reuse is stopped.
    Ambiguous,
}

/// Safe outcome of one explicit or scheduled refresh attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuth2RefreshOutcome {
    /// No managed credential exists for the selected Provider.
    NotConfigured,
    /// The persisted credential was already outside the safety window.
    Current {
        /// Snapshot generation current after guarded reload.
        generation: u64,
    },
    /// One complete rotated bundle was persisted and published.
    Refreshed {
        /// Newly published snapshot generation.
        generation: u64,
    },
    /// A confirmed transient failure is scheduled for bounded retry.
    Backoff {
        /// Current snapshot generation retained during backoff.
        generation: u64,
    },
    /// Explicit login is required before automatic refresh can resume.
    ReauthRequired {
        /// Current snapshot generation retained at the terminal failure.
        generation: u64,
    },
    /// The refresh result may have rotated and cannot be safely retried.
    Ambiguous {
        /// Current snapshot generation retained without publishing unpersisted tokens.
        generation: u64,
    },
}

/// Value-free failure returned when a request cannot borrow a usable OAuth2 generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OAuth2CredentialLeaseError {
    /// No managed credential exists for the selected Provider.
    NotConfigured,
    /// The credential is expired, rotating, backing off, or in a terminal lifecycle state.
    Unavailable,
}

/// Short-lived owned access-token and account-context snapshot for one Provider request.
pub(crate) struct OAuth2CredentialLease {
    store: CredentialStore,
    provider: ProviderKind,
    pool_id: String,
    generation: u64,
}

impl OAuth2CredentialLease {
    /// Returns the compile-time credential binding captured by this lease.
    pub(crate) fn pool_id(&self) -> &str {
        &self.pool_id
    }

    /// Returns the non-sensitive lifecycle generation captured by this lease.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Borrows the sole account-bound credential through the standard Provider egress view.
    pub(crate) fn credential(&self) -> Result<UpstreamCredential<'_>, CredentialStoreError> {
        self.store
            .upstream_pool(
                self.provider,
                &self.pool_id,
                CredentialKind::OAuth2BearerAccessToken,
            )?
            .into_iter()
            .next()
            .ok_or(CredentialStoreError::Unavailable)
    }
}

impl fmt::Debug for OAuth2CredentialLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2CredentialLease")
            .field("provider", &self.provider)
            .field("pool_id", &self.pool_id)
            .field("generation", &self.generation)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

pub(super) struct ManagedOAuth2Credential {
    pub(super) provider: ProviderKind,
    pub(super) pool_id: String,
    member_id: String,
    pub(super) target: OAuth2LoginTarget,
    state: Mutex<ManagedOAuth2State>,
    pub(super) refresh_gate: AsyncMutex<()>,
}

impl ManagedOAuth2Credential {
    pub(super) fn new(
        provider: ProviderKind,
        pool_id: &str,
        path: PathBuf,
        bundle: ValidatedOAuth2Bundle,
        version: OAuth2AuthFileVersion,
    ) -> Self {
        Self {
            provider,
            pool_id: pool_id.to_owned(),
            member_id: format!("{pool_id}#1"),
            target: OAuth2LoginTarget::new(provider, pool_id, path),
            state: Mutex::new(ManagedOAuth2State {
                bundle,
                version,
                generation: 1,
                status: OAuth2CredentialStatus::Active,
                consecutive_failures: 0,
                next_attempt: None,
            }),
            refresh_gate: AsyncMutex::new(()),
        }
    }
    /// Captures one atomic redacted view of mutable lifecycle state.
    pub(super) fn snapshot(&self) -> OAuth2Credential {
        let state = self.lock_state();
        OAuth2Credential {
            provider: self.provider,
            pool_id: self.pool_id.clone(),
            member_id: self.member_id.clone(),
            metadata: oauth2_metadata(state.generation, state.bundle.expires_at),
            status: state.status,
        }
    }

    /// Returns the current generation without borrowing credential material.
    pub(super) fn current_generation(&self) -> u64 {
        self.lock_state().generation
    }

    /// Returns the generation and persisted version atomically for guarded recovery decisions.
    pub(super) fn current_generation_and_version(&self) -> (u64, OAuth2AuthFileVersion) {
        let state = self.lock_state();
        (state.generation, state.version.clone())
    }

    /// Reports whether a request must join scheduled refresh before borrowing a token.
    pub(super) fn requires_request_refresh(&self, now: SystemTime) -> bool {
        let state = self.lock_state();
        match state.status {
            OAuth2CredentialStatus::Active => {
                refresh_due_at(&self.pool_id, state.bundle.expires_at) <= now
            }
            OAuth2CredentialStatus::RefreshBackoff => {
                state.next_attempt.is_some_and(|attempt| attempt <= now)
            }
            OAuth2CredentialStatus::ReauthRequired | OAuth2CredentialStatus::Ambiguous => false,
        }
    }

    /// Copies only current egress material into an owned one-member request lease.
    pub(super) fn lease(
        &self,
        now: SystemTime,
    ) -> Result<OAuth2CredentialLease, OAuth2CredentialLeaseError> {
        // Reject terminal, backoff, and expired state before copying any secret material.
        let state = self.lock_state();
        if state.status != OAuth2CredentialStatus::Active || state.bundle.expires_at <= now {
            return Err(OAuth2CredentialLeaseError::Unavailable);
        }

        // Build the same purpose-bound credential shape consumed by the Provider adapter.
        let mut builder = CredentialStoreBuilder::new();
        builder
            .insert_chatgpt_oauth_member(
                self.pool_id.clone(),
                self.member_id.clone(),
                SecretString::from(state.bundle.access_token.expose_secret().to_owned()),
                SecretString::from(state.bundle.account_id.expose_secret().to_owned()),
                state.bundle.is_fedramp_account,
                oauth2_metadata(state.generation, state.bundle.expires_at),
            )
            .map_err(|_| OAuth2CredentialLeaseError::Unavailable)?;
        Ok(OAuth2CredentialLease {
            store: builder.build(),
            provider: self.provider,
            pool_id: self.pool_id.clone(),
            generation: state.generation,
        })
    }

    /// Returns terminal/backoff state before any file or network operation.
    pub(super) fn current_terminal_or_backoff(
        &self,
        now: SystemTime,
    ) -> Option<OAuth2RefreshOutcome> {
        let state = self.lock_state();
        match state.status {
            OAuth2CredentialStatus::ReauthRequired => Some(OAuth2RefreshOutcome::ReauthRequired {
                generation: state.generation,
            }),
            OAuth2CredentialStatus::Ambiguous => Some(OAuth2RefreshOutcome::Ambiguous {
                generation: state.generation,
            }),
            OAuth2CredentialStatus::RefreshBackoff
                if state.next_attempt.is_some_and(|attempt| attempt > now) =>
            {
                Some(OAuth2RefreshOutcome::Backoff {
                    generation: state.generation,
                })
            }
            _ => None,
        }
    }

    /// Publishes a guarded reload only when another writer changed the persisted version.
    pub(super) fn publish_current_if_changed(
        &self,
        bundle: ValidatedOAuth2Bundle,
        version: OAuth2AuthFileVersion,
    ) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        if state.version != version {
            state.generation = state.generation.saturating_add(1);
            state.bundle = bundle;
            state.version = version;
        }
        state.status = OAuth2CredentialStatus::Active;
        state.consecutive_failures = 0;
        state.next_attempt = None;
        OAuth2RefreshOutcome::Current {
            generation: state.generation,
        }
    }

    /// Publishes one fully persisted refreshed bundle and resets failure state.
    pub(super) fn publish_refreshed(
        &self,
        bundle: ValidatedOAuth2Bundle,
        version: OAuth2AuthFileVersion,
    ) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        state.generation = state.generation.saturating_add(1);
        state.bundle = bundle;
        state.version = version;
        state.status = OAuth2CredentialStatus::Active;
        state.consecutive_failures = 0;
        state.next_attempt = None;
        OAuth2RefreshOutcome::Refreshed {
            generation: state.generation,
        }
    }

    /// Records a transport classification without retaining response or credential values.
    pub(super) fn record_transport_failure(
        &self,
        error: RefreshTransportError,
        now: SystemTime,
    ) -> OAuth2RefreshOutcome {
        match error {
            RefreshTransportError::Transient { retry_after } => {
                self.record_backoff(now, retry_after)
            }
            RefreshTransportError::ReauthRequired(_) => self.record_reauth_required(),
            RefreshTransportError::Ambiguous => self.record_ambiguous(),
        }
    }

    /// Treats pre-request local storage failures as bounded transient failures.
    pub(super) fn record_storage_failure(&self, now: SystemTime) -> OAuth2RefreshOutcome {
        self.record_backoff(now, None)
    }

    /// Records bounded exponential backoff for a confirmed retryable failure.
    pub(super) fn record_backoff(
        &self,
        now: SystemTime,
        retry_after: Option<Duration>,
    ) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        let exponent = state.consecutive_failures.saturating_sub(1).min(4);
        let fallback = INITIAL_BACKOFF
            .checked_mul(1_u32 << exponent)
            .unwrap_or(MAX_BACKOFF)
            .min(MAX_BACKOFF);
        let delay = retry_after
            .unwrap_or(fallback)
            .max(Duration::from_secs(1))
            .min(MAX_BACKOFF);
        state.status = OAuth2CredentialStatus::RefreshBackoff;
        state.next_attempt = now.checked_add(delay);
        OAuth2RefreshOutcome::Backoff {
            generation: state.generation,
        }
    }

    /// Stops automatic refresh after a terminal authority rejection or invalid local source.
    pub(super) fn record_reauth_required(&self) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        state.status = OAuth2CredentialStatus::ReauthRequired;
        state.next_attempt = None;
        OAuth2RefreshOutcome::ReauthRequired {
            generation: state.generation,
        }
    }

    /// Marks only the currently rejected replay generation as requiring explicit login.
    pub(super) fn record_reauth_required_if_current(
        &self,
        rejected_generation: u64,
    ) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        if state.generation != rejected_generation {
            return OAuth2RefreshOutcome::Current {
                generation: state.generation,
            };
        }
        state.status = OAuth2CredentialStatus::ReauthRequired;
        state.next_attempt = None;
        OAuth2RefreshOutcome::ReauthRequired {
            generation: state.generation,
        }
    }

    /// Stops automatic reuse when a rotating refresh result cannot be known or persisted.
    pub(super) fn record_ambiguous(&self) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        state.status = OAuth2CredentialStatus::Ambiguous;
        state.next_attempt = None;
        OAuth2RefreshOutcome::Ambiguous {
            generation: state.generation,
        }
    }

    /// Reports whether the active due time or transient retry deadline has elapsed.
    pub(super) fn is_due(&self, now: SystemTime) -> bool {
        self.next_due().is_some_and(|due| due <= now)
    }

    /// Returns the next active refresh or transient retry deadline.
    pub(super) fn next_due(&self) -> Option<SystemTime> {
        let state = self.lock_state();
        match state.status {
            OAuth2CredentialStatus::Active => {
                Some(refresh_due_at(&self.pool_id, state.bundle.expires_at))
            }
            OAuth2CredentialStatus::RefreshBackoff => state.next_attempt,
            OAuth2CredentialStatus::ReauthRequired | OAuth2CredentialStatus::Ambiguous => None,
        }
    }

    /// Recovers state after a poisoned test or application panic without exposing secrets.
    pub(super) fn lock_state(&self) -> MutexGuard<'_, ManagedOAuth2State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(super) struct ManagedOAuth2State {
    pub(super) bundle: ValidatedOAuth2Bundle,
    pub(super) version: OAuth2AuthFileVersion,
    pub(super) generation: u64,
    pub(super) status: OAuth2CredentialStatus,
    pub(super) consecutive_failures: u32,
    pub(super) next_attempt: Option<SystemTime>,
}

/// Constructs non-sensitive metadata for one published lifecycle generation.
fn oauth2_metadata(generation: u64, expires_at: SystemTime) -> CredentialMetadata {
    CredentialMetadata::upstream(
        CredentialKind::OAuth2BearerAccessToken,
        CredentialSource::OAuth2AuthJsonFile,
    )
    .with_generation(generation)
    .with_expires_at(expires_at)
}

/// Derives an early, deterministic per-pool due time without postponing expiry.
pub(super) fn refresh_due_at(pool_id: &str, expires_at: SystemTime) -> SystemTime {
    let hash = pool_id
        .as_bytes()
        .iter()
        .fold(2_166_136_261_u32, |hash, byte| {
            hash.wrapping_mul(16_777_619) ^ u32::from(*byte)
        });
    let jitter = Duration::from_secs(u64::from(hash) % (MAX_EARLY_JITTER + 1));
    expires_at
        .checked_sub(REFRESH_SAFETY_WINDOW.saturating_add(jitter))
        .unwrap_or(UNIX_EPOCH)
}
