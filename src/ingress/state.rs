//! Shared service state used by HTTP ingress.

use std::sync::Arc;

use crate::{
    credential::CredentialStore, identity::UserRegistry, observability::GatewayMetrics,
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
        Self {
            registry,
            upstream,
            users,
            credentials,
            health: Arc::new(TargetHealth::default()),
            credential_health: Arc::new(CredentialHealth::default()),
            metrics: GatewayMetrics::default(),
        }
    }

    /// Returns the shared in-process low-cardinality counter handle for exporter or test snapshots.
    pub fn metrics(&self) -> GatewayMetrics {
        self.metrics.clone()
    }
}
