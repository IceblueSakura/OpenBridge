//! Minimal HTTP ingress over immutable, explicitly bound text entries.
mod admission;
mod auth;
mod body;
pub mod bootstrap;
mod config;
mod diagnostics;
mod error;
mod http;
#[cfg(test)]
mod tests;
use crate::{
    adapter::Adapter,
    lowering::generation::GenerationRepresentationContract,
    protocol::openai::Profile,
    provider::{ProviderDefinition, SecretMaterial},
    topology::{Endpoint, PublicModel},
    transport::http::HttpTransport,
};
pub use config::{Credentials, Entry, Limits, StartupError};
use error::ApiError;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{Semaphore, watch};
struct BoundCandidate {
    endpoint: Endpoint,
    provider: ProviderDefinition,
    secret: Arc<SecretMaterial>,
}
struct BoundEntry {
    public: PublicModel,
    client: Adapter,
    downstream: GenerationRepresentationContract,
    policy: crate::topology::RoutePolicy,
    candidates: Vec<Arc<BoundCandidate>>,
}
impl BoundEntry {
    fn eligible(
        &self,
        request: &crate::adapter::Request,
    ) -> Result<Vec<Arc<BoundCandidate>>, ApiError> {
        // No shared model scope can authorize another issuer's opaque continuation.
        if self.candidates.len() > 1
            && (request.task.semantic.reasoning().encrypted_output()
                || request
                    .task
                    .semantic
                    .items()
                    .iter()
                    .any(|(id, _)| request.task.fidelity.replay(*id).is_some()))
        {
            return Err(ApiError::invalid());
        }
        let mut result = vec![];
        for candidate in &self.candidates {
            if crate::execution::plan::representable(&candidate.endpoint, request) {
                result.push(candidate.clone());
            } else if self.policy.candidates == crate::topology::CandidatePolicy::RequireAll {
                return Err(ApiError::invalid());
            }
        }
        if result.is_empty() {
            return Err(ApiError::invalid());
        }
        Ok(result)
    }
}
struct Runtime {
    diagnostics: Option<diagnostics::Sink>,
    auth: auth::Auth,
    entries: BTreeMap<(u8, String), Arc<BoundEntry>>,
    limits: Limits,
    permits: Arc<Semaphore>,
    transport: HttpTransport,
    shutdown: watch::Sender<bool>,
}
#[derive(Clone)]
pub struct Gateway {
    state: Arc<Runtime>,
}
fn family(profile: Profile) -> u8 {
    match profile {
        Profile::Chat => 0,
        Profile::Responses => 1,
    }
}
