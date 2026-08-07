//! Shared service state used by HTTP ingress.

use std::sync::Arc;

use crate::{
    credential::CredentialStore, identity::UserRegistry,
    oauth2_credentials::OAuth2CredentialManager, observability::GatewayMetrics,
    registry::RuntimeRegistry, transport::upstream::UpstreamTransport,
};

use super::{credential_health::CredentialHealth, health::TargetHealth};

/// Immutable service handles required by handlers.
///
/// The compile-time registry remains immutable after startup. Upstream transport and credential
/// sources are injected as traits/value objects, allowing contract tests to verify HTTP/SSE
/// boundaries without a real Provider or plaintext environment secret.
#[derive(Clone)]
pub struct GatewayState {
    pub(super) registry: Arc<RuntimeRegistry>,
    pub(super) upstream: Arc<dyn UpstreamTransport>,
    pub(super) users: Arc<UserRegistry>,
    pub(super) credentials: Arc<CredentialStore>,
    oauth2_credentials: Arc<OAuth2CredentialManager>,
    pub(super) health: Arc<TargetHealth>,
    pub(super) credential_health: Arc<CredentialHealth>,
    pub(super) metrics: GatewayMetrics,
}

impl GatewayState {
    /// Creates service state with injectable transport and credential sources.
    pub fn new(
        registry: Arc<RuntimeRegistry>,
        upstream: Arc<dyn UpstreamTransport>,
        users: Arc<UserRegistry>,
        credentials: Arc<CredentialStore>,
    ) -> Self {
        Self::new_with_oauth2_credentials(
            registry,
            upstream,
            users,
            credentials,
            Arc::new(OAuth2CredentialManager::empty()),
        )
    }

    /// Creates service state with the shared, internally guarded OAuth2 credential manager.
    pub fn new_with_oauth2_credentials(
        registry: Arc<RuntimeRegistry>,
        upstream: Arc<dyn UpstreamTransport>,
        users: Arc<UserRegistry>,
        credentials: Arc<CredentialStore>,
        oauth2_credentials: Arc<OAuth2CredentialManager>,
    ) -> Self {
        Self {
            registry,
            upstream,
            users,
            credentials,
            oauth2_credentials,
            health: Arc::new(TargetHealth::default()),
            credential_health: Arc::new(CredentialHealth::default()),
            metrics: GatewayMetrics::default(),
        }
    }

    /// Returns the shared OAuth2 lifecycle manager for trusted runtime composition.
    pub fn oauth2_credentials(&self) -> &OAuth2CredentialManager {
        &self.oauth2_credentials
    }

    /// Replaces no-op instruments with the startup-owned OpenTelemetry meter instruments.
    pub fn with_metrics(mut self, metrics: GatewayMetrics) -> Self {
        self.metrics = metrics;
        self
    }
}
