//! Startup-only resolution of immutable entry, contract and credential ownership.
use super::{BoundCandidate, BoundEntry, Gateway, Runtime, auth, family};
use crate::{
    adapter::{Adapter, Dialect},
    lowering::generation::GenerationRepresentationContract,
    protocol::openai::Profile,
    provider::{CredentialBindingId, SecretMaterial},
    semantic::value::ReplayOrigin,
    topology::{CompiledTopology, EndpointId},
    transport::http::HttpTransport,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::{Semaphore, watch};
#[derive(Clone, Debug)]
pub struct Entry {
    pub model: String,
    pub protocol: Profile,
    pub endpoint: EndpointId,
}
pub type Credentials = BTreeMap<CredentialBindingId, Arc<SecretMaterial>>;
#[derive(Clone, Debug)]
pub struct Limits {
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub event_bytes: usize,
    pub max_events: usize,
    pub concurrency: usize,
    pub body_timeout: Duration,
    pub exchange_timeout: Duration,
    pub default_output_tokens: u64,
    pub max_output_tokens: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            request_bytes: 256 << 10,
            response_bytes: 8 << 20,
            event_bytes: 1 << 20,
            max_events: 65_536,
            concurrency: 16,
            body_timeout: Duration::from_secs(10),
            exchange_timeout: Duration::from_secs(120),
            default_output_tokens: 1024,
            max_output_tokens: 16384,
        }
    }
}
impl Limits {
    fn validate(&self) -> bool {
        (1..=4 << 20).contains(&self.request_bytes)
            && (1..=64 << 20).contains(&self.response_bytes)
            && (1..=4 << 20).contains(&self.event_bytes)
            && (1..=1_000_000).contains(&self.max_events)
            && (1..=256).contains(&self.concurrency)
            && !self.body_timeout.is_zero()
            && self.body_timeout <= Duration::from_secs(60)
            && !self.exchange_timeout.is_zero()
            && self.exchange_timeout <= Duration::from_secs(300)
            && self.default_output_tokens > 0
            && self.default_output_tokens <= self.max_output_tokens
            && self.max_output_tokens <= 1_000_000
    }
}
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum StartupError {
    #[error("invalid gateway limits")]
    Limits,
    #[error("missing or invalid credential configuration")]
    Credentials,
    #[error("invalid or duplicate fixed entry binding")]
    Binding,
    #[error("invalid HTTP transport configuration")]
    Transport,
    #[error("invalid loopback listener configuration")]
    Listener,
    #[error("invalid startup environment")]
    Environment,
}
impl Gateway {
    /// Enable bounded metadata in a new private operator file, before sharing the gateway.
    pub fn with_probe_diagnostics(mut self, path: &std::path::Path) -> Result<Self, StartupError> {
        let state = Arc::get_mut(&mut self.state).ok_or(StartupError::Environment)?;
        state.diagnostics =
            Some(super::diagnostics::Sink::open(path).map_err(|_| StartupError::Environment)?);
        Ok(self)
    }
    /// Best-effort bounded drain, for an owner shutting down its test listener.
    pub async fn flush_probe_diagnostics(&self) {
        if let Some(sink) = &self.state.diagnostics {
            sink.flush().await;
        }
    }
    pub fn new(
        topology: CompiledTopology,
        entries: Vec<Entry>,
        credentials: Credentials,
        client_key: SecretMaterial,
        limits: Limits,
        proxy: Option<&str>,
    ) -> Result<Self, StartupError> {
        if !limits.validate() {
            return Err(StartupError::Limits);
        }
        let auth = auth::Auth::new(client_key)?;
        let transport = HttpTransport::new(proxy).map_err(|_| StartupError::Transport)?;
        let mut bound: BTreeMap<(u8, String), BoundEntry> = BTreeMap::new();
        for entry in entries {
            let public = topology
                .model(&entry.model)
                .ok_or(StartupError::Binding)?
                .clone();
            let route = topology.route(&public.route).ok_or(StartupError::Binding)?;
            if !route.endpoints.contains(&entry.endpoint) {
                return Err(StartupError::Binding);
            }
            let mut endpoint = topology
                .endpoint(&entry.endpoint)
                .ok_or(StartupError::Binding)?
                .clone();
            let provider = topology
                .provider(endpoint.provider.as_str())
                .ok_or(StartupError::Binding)?
                .clone();
            let secret = credentials
                .get(&endpoint.credential)
                .ok_or(StartupError::Credentials)?
                .clone();
            // Length prefixes avoid ambiguous concatenation. Internal provenance
            // is neither a credential locator nor attestation of token issuance.
            let mut hash = Sha256::new();
            hash.update(auth.digest);
            for value in [
                secret.expose(),
                public.id.as_str(),
                provider.id.as_str(),
                endpoint.upstream_model.as_str(),
            ] {
                hash.update((value.len() as u64).to_le_bytes());
                hash.update(value.as_bytes());
            }
            let digest: String = hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let scope = ReplayOrigin::new(&format!("gateway-{digest}"))
                .map_err(|_| StartupError::Binding)?;
            endpoint.representation.adaptation.scope = Some(scope.clone());
            if endpoint.representation.replay_origin.is_some() {
                endpoint.representation.replay_origin = Some(scope.clone());
            }
            let client = Adapter::new(entry.protocol, Dialect::OpenBridge, Some(scope.clone()));
            // Model input admission must not filter facts reported on the client wire
            // (e.g. empty logprobs or defaults a model cannot accept as controls).
            let downstream = GenerationRepresentationContract {
                replay_origin: Some(scope),
                reported_facts: public.reported_facts,
                ..GenerationRepresentationContract::full()
            };
            let group = bound
                .entry((family(entry.protocol), entry.model))
                .or_insert_with(|| BoundEntry {
                    public,
                    client,
                    downstream,
                    policy: route.policy.clone(),
                    candidates: vec![],
                });
            if group
                .candidates
                .iter()
                .any(|c| c.endpoint.id == endpoint.id)
            {
                return Err(StartupError::Binding);
            }
            group.candidates.push(Arc::new(BoundCandidate {
                endpoint,
                provider,
                secret,
            }));
        }
        if bound.is_empty() {
            return Err(StartupError::Binding);
        }
        let mut activated = BTreeMap::new();
        for (key, mut group) in bound {
            let route = topology
                .route(&group.public.route)
                .ok_or(StartupError::Binding)?;
            group.candidates.sort_by_key(|c| {
                route
                    .endpoints
                    .iter()
                    .position(|id| id == &c.endpoint.id)
                    .expect("validated member")
            });
            if group.candidates.len() > 1 {
                group.client.adaptation.scope = None;
                group.downstream.replay_origin = None;
            }
            activated.insert(key, Arc::new(group));
        }
        let (shutdown, _) = watch::channel(false);
        let permits = Arc::new(Semaphore::new(limits.concurrency));
        Ok(Self {
            state: Arc::new(Runtime {
                diagnostics: None,
                auth,
                entries: activated,
                limits,
                permits,
                transport,
                shutdown,
            }),
        })
    }
}
