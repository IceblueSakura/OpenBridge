//! Runtime ownership, snapshot publication, and scheduling for managed OAuth2 credentials.
//!
//! Each Provider has one in-process refresh gate and one cross-process auth-file lock. Callers see
//! owned redacted snapshots; only the lifecycle state machine can borrow tokens or file locators.

use std::{
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use secrecy::ExposeSecret;
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    credential::{CredentialMetadata, CredentialSource},
    provider::{CredentialKind, ProviderKind},
    providers::chatgpt::oauth::REGISTRATION,
};

pub use super::document::OAuth2CredentialManagerError;
use super::{
    OAuth2LoginTarget,
    document::{
        ValidatedOAuth2Bundle, parse_auth_document, serialize_auth_document,
        validate_refreshed_tokens,
    },
    refresh::{ChatGptRefreshTransport, RefreshTransportError, ReqwestChatGptRefreshTransport},
    storage::{
        OAuth2AuthFileVersion, OAuth2StorageError, read_auth_document, version_for_document,
    },
};

const REFRESH_SAFETY_WINDOW: Duration = Duration::from_secs(120);
const MAX_EARLY_JITTER: u64 = 30;
const INITIAL_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(5 * 60);
const IDLE_SCHEDULER_WAKE: Duration = Duration::from_secs(24 * 60 * 60);

/// Collection of Provider-bound OAuth2 credentials with guarded refresh state.
pub struct OAuth2CredentialManager {
    credentials: Vec<Arc<ManagedOAuth2Credential>>,
}

impl OAuth2CredentialManager {
    /// Creates an empty manager for runtimes and tests with no configured OAuth2 Provider.
    pub fn empty() -> Self {
        Self {
            credentials: Vec::new(),
        }
    }

    /// Returns the number of configured OAuth2 Providers without exposing locators or tokens.
    pub fn configured_provider_count(&self) -> usize {
        self.credentials.len()
    }

    /// Returns an owned, redacted snapshot for one Provider, if configured.
    pub fn credential_for_provider(&self, provider: ProviderKind) -> Option<OAuth2Credential> {
        self.credentials
            .iter()
            .find(|credential| credential.provider == provider)
            .map(|credential| credential.snapshot())
    }

    /// Refreshes one configured Provider now when its persisted token is due.
    pub async fn refresh_provider(&self, provider: ProviderKind) -> OAuth2RefreshOutcome {
        // Resolve the fixed Provider transport without accepting runtime endpoint overrides.
        let Some(credential) = self.find_credential(provider) else {
            return OAuth2RefreshOutcome::NotConfigured;
        };
        let transport = match ReqwestChatGptRefreshTransport::new(&REGISTRATION) {
            Ok(transport) => transport,
            Err(error) => {
                return credential.record_transport_failure(error, SystemTime::now());
            }
        };

        // Execute the guarded lifecycle against the current wall-clock instant.
        self.refresh_provider_with(credential, &transport, SystemTime::now())
            .await
    }

    /// Runs the expiry-driven refresh scheduler until its task is cancelled by the composition root.
    pub async fn run_refresh_scheduler(self: Arc<Self>) {
        // Recompute the earliest due time after every wake or completed refresh.
        loop {
            let now = SystemTime::now();
            let delay = self.next_scheduler_delay(now);
            tokio::time::sleep(delay).await;

            // Refresh each independently due Provider once; each credential owns its single-flight.
            let now = SystemTime::now();
            for credential in self.due_credentials(now) {
                let transport = match ReqwestChatGptRefreshTransport::new(&REGISTRATION) {
                    Ok(transport) => transport,
                    Err(error) => {
                        let outcome = credential.record_transport_failure(error, SystemTime::now());
                        tracing::warn!(
                            provider = ?credential.provider,
                            pool_id = %credential.pool_id,
                            status = ?outcome,
                            "OAuth2 credential refresh client could not be created"
                        );
                        continue;
                    }
                };
                let outcome = self
                    .refresh_provider_with(credential, &transport, now)
                    .await;
                match outcome {
                    OAuth2RefreshOutcome::Refreshed { generation } => tracing::info!(
                        provider = ?credential.provider,
                        pool_id = %credential.pool_id,
                        generation,
                        "OAuth2 credential refresh completed"
                    ),
                    OAuth2RefreshOutcome::Current { .. } => {}
                    _ => tracing::warn!(
                        provider = ?credential.provider,
                        pool_id = %credential.pool_id,
                        status = ?outcome,
                        "OAuth2 credential refresh did not complete"
                    ),
                }
            }
        }
    }

    /// Runs one in-process and cross-process guarded refresh using a replaceable transport.
    async fn refresh_provider_with<T>(
        &self,
        credential: &Arc<ManagedOAuth2Credential>,
        transport: &T,
        now: SystemTime,
    ) -> OAuth2RefreshOutcome
    where
        T: ChatGptRefreshTransport,
    {
        // Merge concurrent callers for the same credential before acquiring the file lock.
        let _refresh_gate = credential.refresh_gate.lock().await;
        if let Some(outcome) = credential.current_terminal_or_backoff(now) {
            return outcome;
        }

        // Acquire the cross-process lock and reload the complete persisted source under it.
        let target = credential.target.clone();
        let locked_source = tokio::task::spawn_blocking(move || {
            let locked = target.lock()?;
            let document = locked.read_document()?;
            let version = version_for_document(&document);
            Ok::<_, OAuth2StorageError>((locked, document, version))
        })
        .await;
        let (locked, document, version) = match locked_source {
            Ok(Ok(source)) => source,
            Ok(Err(_)) | Err(_) => return credential.record_storage_failure(now),
        };
        let persisted = match parse_auth_document(&document, false) {
            Ok(bundle) => bundle,
            Err(_) => return credential.record_reauth_required(),
        };

        // Skip the network when another worker already published a token outside the safety window.
        if refresh_due_at(&credential.pool_id, persisted.expires_at) > now {
            return credential.publish_current_if_changed(persisted, version);
        }

        // Send exactly one refresh grant while retaining the cross-process rotation lease.
        let response = match transport.refresh(&persisted.refresh_token).await {
            Ok(response) => response,
            Err(error) => return credential.record_transport_failure(error, now),
        };
        if response.validate_token_type().is_err() {
            return credential.record_ambiguous();
        }
        let refreshed = match validate_refreshed_tokens(
            &persisted,
            response
                .id_token
                .as_ref()
                .map(secrecy::ExposeSecret::expose_secret),
            response.access_token.expose_secret(),
            response
                .refresh_token
                .as_ref()
                .map(secrecy::ExposeSecret::expose_secret),
        ) {
            Ok(bundle) => bundle,
            Err(_) => return credential.record_ambiguous(),
        };
        let serialized = match serialize_auth_document(&refreshed) {
            Ok(document) => document,
            Err(_) => return credential.record_ambiguous(),
        };
        let next_version = version_for_document(&serialized);

        // Persist the complete rotation in the held transaction before publishing memory state.
        let write = tokio::task::spawn_blocking(move || locked.replace(&serialized)).await;
        match write {
            Ok(Ok(())) => credential.publish_refreshed(refreshed, next_version),
            Ok(Err(_)) | Err(_) => credential.record_ambiguous(),
        }
    }

    /// Finds the sole managed entry for a Provider.
    fn find_credential(&self, provider: ProviderKind) -> Option<&Arc<ManagedOAuth2Credential>> {
        self.credentials
            .iter()
            .find(|credential| credential.provider == provider)
    }

    /// Returns credentials whose active due time or transient backoff has elapsed.
    fn due_credentials(&self, now: SystemTime) -> Vec<&Arc<ManagedOAuth2Credential>> {
        self.credentials
            .iter()
            .filter(|credential| credential.is_due(now))
            .collect()
    }

    /// Computes a bounded scheduler sleep and wakes daily for otherwise terminal/empty state.
    fn next_scheduler_delay(&self, now: SystemTime) -> Duration {
        self.credentials
            .iter()
            .filter_map(|credential| credential.next_due())
            .map(|due| due.duration_since(now).unwrap_or(Duration::ZERO))
            .min()
            .unwrap_or(IDLE_SCHEDULER_WAKE)
            .min(IDLE_SCHEDULER_WAKE)
    }
}

impl Default for OAuth2CredentialManager {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for OAuth2CredentialManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2CredentialManager")
            .field("configured_providers", &self.credentials.len())
            .finish()
    }
}

/// Owned, redacted snapshot of one managed OAuth2 credential.
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

struct ManagedOAuth2Credential {
    provider: ProviderKind,
    pool_id: String,
    member_id: String,
    target: OAuth2LoginTarget,
    state: Mutex<ManagedOAuth2State>,
    refresh_gate: AsyncMutex<()>,
}

impl ManagedOAuth2Credential {
    /// Captures one atomic redacted view of mutable lifecycle state.
    fn snapshot(&self) -> OAuth2Credential {
        let state = self.lock_state();
        OAuth2Credential {
            provider: self.provider,
            pool_id: self.pool_id.clone(),
            member_id: self.member_id.clone(),
            metadata: oauth2_metadata(state.generation, state.bundle.expires_at),
            status: state.status,
        }
    }

    /// Returns terminal/backoff state before any file or network operation.
    fn current_terminal_or_backoff(&self, now: SystemTime) -> Option<OAuth2RefreshOutcome> {
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
    fn publish_current_if_changed(
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
    fn publish_refreshed(
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
    fn record_transport_failure(
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
    fn record_storage_failure(&self, now: SystemTime) -> OAuth2RefreshOutcome {
        self.record_backoff(now, None)
    }

    /// Records bounded exponential backoff for a confirmed retryable failure.
    fn record_backoff(
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
    fn record_reauth_required(&self) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        state.status = OAuth2CredentialStatus::ReauthRequired;
        state.next_attempt = None;
        OAuth2RefreshOutcome::ReauthRequired {
            generation: state.generation,
        }
    }

    /// Stops automatic reuse when a rotating refresh result cannot be known or persisted.
    fn record_ambiguous(&self) -> OAuth2RefreshOutcome {
        let mut state = self.lock_state();
        state.status = OAuth2CredentialStatus::Ambiguous;
        state.next_attempt = None;
        OAuth2RefreshOutcome::Ambiguous {
            generation: state.generation,
        }
    }

    /// Reports whether the active due time or transient retry deadline has elapsed.
    fn is_due(&self, now: SystemTime) -> bool {
        self.next_due().is_some_and(|due| due <= now)
    }

    /// Returns the next active refresh or transient retry deadline.
    fn next_due(&self) -> Option<SystemTime> {
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
    fn lock_state(&self) -> MutexGuard<'_, ManagedOAuth2State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

struct ManagedOAuth2State {
    bundle: ValidatedOAuth2Bundle,
    version: OAuth2AuthFileVersion,
    generation: u64,
    status: OAuth2CredentialStatus,
    consecutive_failures: u32,
    next_attempt: Option<SystemTime>,
}

/// Startup builder that validates complete files while preserving expired refreshable bundles.
#[derive(Default)]
pub(crate) struct OAuth2CredentialManagerBuilder {
    credentials: Vec<Arc<ManagedOAuth2Credential>>,
}

impl OAuth2CredentialManagerBuilder {
    /// Creates an empty startup builder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Loads one Provider-bound auth file and retains its complete refreshable token bundle.
    pub(crate) fn load_auth_json_file(
        &mut self,
        provider: ProviderKind,
        pool_id: &str,
        path: PathBuf,
    ) -> Result<(), OAuth2CredentialManagerError> {
        // Reject unsupported or duplicate Provider ownership before reading any locator.
        if provider != ProviderKind::ChatGpt {
            return Err(OAuth2CredentialManagerError::UnsupportedProvider);
        }
        if self
            .credentials
            .iter()
            .any(|credential| credential.provider == provider)
        {
            return Err(OAuth2CredentialManagerError::DuplicateProvider);
        }

        // Read and validate complete document shape while allowing an expired access token to refresh.
        let document = read_auth_document(&path).map_err(|_| OAuth2CredentialManagerError::Read)?;
        let bundle = parse_auth_document(&document, false)?;
        let version = version_for_document(&document);

        // Bind the source and mutable lifecycle state to the compile-time Provider identity.
        self.credentials.push(Arc::new(ManagedOAuth2Credential {
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
        }));
        Ok(())
    }

    /// Freezes configured identities while retaining guarded internal lifecycle mutation.
    pub(crate) fn build(self) -> OAuth2CredentialManager {
        OAuth2CredentialManager {
            credentials: self.credentials,
        }
    }
}

impl fmt::Debug for OAuth2CredentialManagerBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2CredentialManagerBuilder")
            .field("configured_providers", &self.credentials.len())
            .finish()
    }
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
fn refresh_due_at(pool_id: &str, expires_at: SystemTime) -> SystemTime {
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use secrecy::{ExposeSecret, SecretString};
    use serde_json::json;
    use tokio::sync::Notify;
    use zeroize::Zeroizing;

    use super::*;
    use crate::oauth2_credentials::refresh::RefreshTokenResponse;

    #[test]
    fn managed_snapshot_retains_complete_tokens_without_exposing_them_through_debug() {
        let fixture = TestDirectory::new();
        let manager = fixture.manager(expiring_document(
            "synthetic-account",
            unix_now() + 3_600,
            "synthetic-refresh",
        ));

        // Confirm the private state owns the complete lifecycle bundle.
        let managed = manager.find_credential(ProviderKind::ChatGpt).unwrap();
        let state = managed.lock_state();
        assert_eq!(
            state.bundle.refresh_token.expose_secret(),
            "synthetic-refresh"
        );
        assert_eq!(state.bundle.account_id.expose_secret(), "synthetic-account");
        drop(state);

        // Keep every secret and locator out of manager and snapshot diagnostics.
        let snapshot = manager
            .credential_for_provider(ProviderKind::ChatGpt)
            .unwrap();
        let debug = format!("{manager:?} {snapshot:?}");
        for forbidden in [
            "synthetic-refresh",
            "synthetic-account",
            "sensitive-auth-file",
        ] {
            assert!(!debug.contains(forbidden));
        }
    }

    #[tokio::test]
    async fn concurrent_refreshes_reload_once_and_publish_one_rotated_generation() {
        let fixture = TestDirectory::new();
        let manager = Arc::new(fixture.manager(expiring_document(
            "synthetic-account",
            1,
            "synthetic-old-refresh",
        )));
        let transport = Arc::new(BlockingTransport::new(RefreshTokenResponse {
            access_token: SecretString::from(jwt(json!({"exp": unix_now() + 3_600}))),
            id_token: None,
            refresh_token: Some(SecretString::from("synthetic-rotated-refresh".to_owned())),
            token_type: Some("Bearer".to_owned()),
        }));
        let now = SystemTime::now();

        // Hold the first network exchange while a second caller queues behind the in-process gate.
        let first_manager = Arc::clone(&manager);
        let first_transport = Arc::clone(&transport);
        let first = tokio::spawn(async move {
            let credential = first_manager
                .find_credential(ProviderKind::ChatGpt)
                .unwrap();
            first_manager
                .refresh_provider_with(credential, &*first_transport, now)
                .await
        });
        transport.started.notified().await;
        let second_manager = Arc::clone(&manager);
        let second_transport = Arc::clone(&transport);
        let second = tokio::spawn(async move {
            let credential = second_manager
                .find_credential(ProviderKind::ChatGpt)
                .unwrap();
            second_manager
                .refresh_provider_with(credential, &*second_transport, now)
                .await
        });
        transport.release.notify_one();
        let (first, second) = (first.await.unwrap(), second.await.unwrap());

        // Publish one rotated generation and let the queued caller reuse the persisted winner.
        assert_eq!(first, OAuth2RefreshOutcome::Refreshed { generation: 2 });
        assert_eq!(second, OAuth2RefreshOutcome::Current { generation: 2 });
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        let snapshot = manager
            .credential_for_provider(ProviderKind::ChatGpt)
            .unwrap();
        assert_eq!(snapshot.metadata().generation(), 2);
        assert_eq!(snapshot.status(), OAuth2CredentialStatus::Active);
    }

    #[tokio::test]
    async fn refresh_preserves_optional_tokens_and_rejects_account_changes() {
        // Preserve both optional tokens when the authority returns only a new access token.
        let fixture = TestDirectory::new();
        let manager = fixture.manager(expiring_document(
            "synthetic-account",
            1,
            "synthetic-preserved-refresh",
        ));
        let transport = ImmediateTransport::success(RefreshTokenResponse {
            access_token: SecretString::from(jwt(json!({"exp": unix_now() + 3_600}))),
            id_token: None,
            refresh_token: None,
            token_type: None,
        });
        let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();
        let outcome = manager
            .refresh_provider_with(credential, &transport, SystemTime::now())
            .await;
        assert_eq!(outcome, OAuth2RefreshOutcome::Refreshed { generation: 2 });
        let document = Zeroizing::new(fs::read(fixture.auth_file()).unwrap());
        let persisted = parse_auth_document(&document, true).unwrap();
        assert_eq!(
            persisted.refresh_token.expose_secret(),
            "synthetic-preserved-refresh"
        );
        assert_eq!(persisted.account_id.expose_secret(), "synthetic-account");

        // Reject a successful response whose replacement ID token changes account identity.
        let fixture = TestDirectory::new();
        let original = expiring_document("synthetic-account", 1, "synthetic-refresh");
        let manager = fixture.manager(original.clone());
        let transport = ImmediateTransport::success(RefreshTokenResponse {
            access_token: SecretString::from(jwt(json!({"exp": unix_now() + 3_600}))),
            id_token: Some(SecretString::from(id_token("synthetic-other-account"))),
            refresh_token: None,
            token_type: Some("Bearer".to_owned()),
        });
        let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();
        let outcome = manager
            .refresh_provider_with(credential, &transport, SystemTime::now())
            .await;
        assert_eq!(outcome, OAuth2RefreshOutcome::Ambiguous { generation: 1 });
        assert_eq!(fs::read(fixture.auth_file()).unwrap(), original);
    }

    #[tokio::test]
    async fn terminal_ambiguous_and_transient_refresh_failures_are_fail_closed() {
        let cases = [
            (
                RefreshTransportError::Transient { retry_after: None },
                OAuth2CredentialStatus::RefreshBackoff,
            ),
            (
                RefreshTransportError::ReauthRequired(
                    crate::oauth2_credentials::refresh::RefreshTerminalReason::InvalidGrant,
                ),
                OAuth2CredentialStatus::ReauthRequired,
            ),
            (
                RefreshTransportError::Ambiguous,
                OAuth2CredentialStatus::Ambiguous,
            ),
        ];

        // Map each value-free transport category to the corresponding scheduler state.
        for (error, expected_status) in cases {
            let fixture = TestDirectory::new();
            let manager = fixture.manager(expiring_document(
                "synthetic-account",
                1,
                "synthetic-refresh",
            ));
            let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();
            let outcome = manager
                .refresh_provider_with(
                    credential,
                    &ImmediateTransport::failure(error),
                    SystemTime::now(),
                )
                .await;
            assert_eq!(
                manager
                    .credential_for_provider(ProviderKind::ChatGpt)
                    .unwrap()
                    .status(),
                expected_status
            );
            assert!(!matches!(outcome, OAuth2RefreshOutcome::Refreshed { .. }));
        }
    }

    #[test]
    fn scheduler_due_time_uses_expiry_window_and_deterministic_early_jitter() {
        let expiry = UNIX_EPOCH + Duration::from_secs(10_000);

        // Derive a stable due time strictly before the safety-window boundary and expiry.
        let first = refresh_due_at("chatgpt-codex", expiry);
        let second = refresh_due_at("chatgpt-codex", expiry);
        assert_eq!(first, second);
        assert!(first <= expiry - REFRESH_SAFETY_WINDOW);
        assert!(first >= expiry - REFRESH_SAFETY_WINDOW - Duration::from_secs(30));
        assert!(first < expiry);
    }

    struct BlockingTransport {
        response: Mutex<Option<RefreshTokenResponse>>,
        calls: AtomicUsize,
        started: Notify,
        release: Notify,
    }

    impl BlockingTransport {
        fn new(response: RefreshTokenResponse) -> Self {
            Self {
                response: Mutex::new(Some(response)),
                calls: AtomicUsize::new(0),
                started: Notify::new(),
                release: Notify::new(),
            }
        }
    }

    impl ChatGptRefreshTransport for BlockingTransport {
        async fn refresh(
            &self,
            _refresh_token: &SecretString,
        ) -> Result<RefreshTokenResponse, RefreshTransportError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.notify_one();
            self.release.notified().await;
            Ok(self.response.lock().unwrap().take().unwrap())
        }
    }

    struct ImmediateTransport {
        result: Mutex<Option<Result<RefreshTokenResponse, RefreshTransportError>>>,
    }

    impl ImmediateTransport {
        fn success(response: RefreshTokenResponse) -> Self {
            Self {
                result: Mutex::new(Some(Ok(response))),
            }
        }

        fn failure(error: RefreshTransportError) -> Self {
            Self {
                result: Mutex::new(Some(Err(error))),
            }
        }
    }

    impl ChatGptRefreshTransport for ImmediateTransport {
        async fn refresh(
            &self,
            _refresh_token: &SecretString,
        ) -> Result<RefreshTokenResponse, RefreshTransportError> {
            self.result.lock().unwrap().take().unwrap()
        }
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openbridge-oauth-manager-test-{}-{}",
                std::process::id(),
                super::super::storage::next_test_id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn auth_file(&self) -> PathBuf {
            self.path.join("sensitive-auth-file.json")
        }

        fn manager(&self, document: Vec<u8>) -> OAuth2CredentialManager {
            fs::write(self.auth_file(), document).unwrap();
            let mut builder = OAuth2CredentialManagerBuilder::new();
            builder
                .load_auth_json_file(ProviderKind::ChatGpt, "chatgpt-codex", self.auth_file())
                .unwrap();
            builder.build()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            // Remove only files created inside this process-unique test directory.
            if let Ok(entries) = fs::read_dir(&self.path) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.path);
        }
    }

    fn expiring_document(account: &str, expiry: u64, refresh_token: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": id_token(account),
                "access_token": jwt(json!({"exp": expiry})),
                "refresh_token": refresh_token,
                "account_id": account
            },
            "last_refresh": "2026-08-05T00:00:00Z"
        }))
        .unwrap()
    }

    fn id_token(account: &str) -> String {
        jwt(json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account,
                "chatgpt_account_is_fedramp": false
            }
        }))
    }

    fn jwt(payload: serde_json::Value) -> String {
        format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(b"{}"),
            URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes()),
            URL_SAFE_NO_PAD.encode(b"signature")
        )
    }

    fn unix_now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}
