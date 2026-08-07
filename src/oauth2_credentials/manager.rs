//! Runtime ownership, snapshot publication, and scheduling for managed OAuth2 credentials.
//!
//! The collection facade and scheduler live here; per-credential state and refresh transactions
//! are kept in child modules so lifecycle changes remain isolated.

use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::provider::ProviderKind;
use crate::providers::chatgpt::oauth::REGISTRATION;

mod builder;
mod credential;
mod refresh;

pub use super::error::OAuth2CredentialManagerError;
use super::transport::refresh::{ChatGptRefreshTransport, ReqwestChatGptRefreshTransport};
pub(crate) use builder::OAuth2CredentialManagerBuilder;
use credential::ManagedOAuth2Credential;
pub use credential::{OAuth2Credential, OAuth2CredentialStatus, OAuth2RefreshOutcome};
pub(crate) use credential::{OAuth2CredentialLease, OAuth2CredentialLeaseError};

const IDLE_SCHEDULER_WAKE: Duration = Duration::from_secs(24 * 60 * 60);

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

    /// Borrows one current account-bound access-token generation for a single Provider request.
    pub(crate) async fn lease_for_request(
        &self,
        provider: ProviderKind,
    ) -> Result<OAuth2CredentialLease, OAuth2CredentialLeaseError> {
        // Resolve the sole Provider-owned credential and refresh it when its safety window is due.
        let credential = self
            .find_credential(provider)
            .ok_or(OAuth2CredentialLeaseError::NotConfigured)?;
        let now = SystemTime::now();
        if credential.requires_request_refresh(now) {
            match self.refresh_provider(provider).await {
                OAuth2RefreshOutcome::Current { .. } | OAuth2RefreshOutcome::Refreshed { .. } => {}
                OAuth2RefreshOutcome::NotConfigured
                | OAuth2RefreshOutcome::Backoff { .. }
                | OAuth2RefreshOutcome::ReauthRequired { .. }
                | OAuth2RefreshOutcome::Ambiguous { .. } => {
                    return Err(OAuth2CredentialLeaseError::Unavailable);
                }
            }
        }

        // Copy only the egress access token and account context into a short-lived owned lease.
        credential.lease(SystemTime::now())
    }

    /// Reloads or rotates the rejected generation once before a guarded request replay.
    pub(crate) async fn recover_after_unauthorized(
        &self,
        provider: ProviderKind,
        rejected_generation: u64,
    ) -> OAuth2RefreshOutcome {
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

        // Force one reload/refresh transaction for only the generation observed by the request.
        self.recover_after_unauthorized_with(
            credential,
            &transport,
            SystemTime::now(),
            rejected_generation,
        )
        .await
    }

    /// Marks a replayed generation terminal only when no newer rotation has already won.
    pub(crate) fn reject_replayed_generation(
        &self,
        provider: ProviderKind,
        rejected_generation: u64,
    ) -> OAuth2RefreshOutcome {
        self.find_credential(provider)
            .map_or(OAuth2RefreshOutcome::NotConfigured, |credential| {
                credential.record_reauth_required_if_current(rejected_generation)
            })
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

    /// Delegates the guarded scheduled refresh transaction to the lifecycle coordinator.
    async fn refresh_provider_with<T>(
        &self,
        credential: &Arc<ManagedOAuth2Credential>,
        transport: &T,
        now: SystemTime,
    ) -> OAuth2RefreshOutcome
    where
        T: ChatGptRefreshTransport,
    {
        refresh::refresh_provider_with(credential, transport, now).await
    }

    /// Delegates guarded unauthorized recovery to the lifecycle coordinator.
    async fn recover_after_unauthorized_with<T>(
        &self,
        credential: &Arc<ManagedOAuth2Credential>,
        transport: &T,
        now: SystemTime,
        rejected_generation: u64,
    ) -> OAuth2RefreshOutcome
    where
        T: ChatGptRefreshTransport,
    {
        refresh::recover_after_unauthorized_with(credential, transport, now, rejected_generation)
            .await
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

#[cfg(test)]
mod tests;
