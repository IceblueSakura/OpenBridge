//! Minimal HTTP ingress over immutable, explicitly bound text entries.
mod admission;
mod auth;
mod body;
pub mod bootstrap;
mod config;
mod credentials;
mod diagnostics;
mod error;
mod exchange;
mod http;
mod intake;
#[cfg(test)]
mod route_fallback_tests;
#[cfg(test)]
mod subscription_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
use crate::provider::SecretMaterial;
use crate::{
    adapter::Adapter,
    lowering::generation::GenerationRepresentationContract,
    protocol::openai::Profile,
    provider::ProviderDefinition,
    topology::{Endpoint, PublicModel},
    transport::http::HttpTransport,
};
pub use config::{Entry, Limits, StartupError};
pub use credentials::Credentials;
use error::ApiError;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{Semaphore, watch};
struct BoundCandidate {
    endpoint: Endpoint,
    provider: ProviderDefinition,
    secret: credentials::Source,
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
                    .any(|(_, item)| matches!(item, crate::semantic::task::generation::Item::Reasoning(r) if r.encrypted.is_some())))
        {
            return Err(ApiError::invalid());
        }
        let selected = crate::execution::plan::select_candidates(
            request,
            self.policy.candidates,
            self.candidates.iter().map(|c| &c.endpoint),
        )
        .map_err(|_| ApiError::invalid())?;
        Ok(selected
            .candidates
            .iter()
            .map(|planned| {
                self.candidates
                    .iter()
                    .find(|c| c.endpoint.id == planned.endpoint_id)
                    .expect("selected activated member")
                    .clone()
            })
            .collect())
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
