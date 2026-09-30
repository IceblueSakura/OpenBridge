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
struct BoundEntry {
    public: PublicModel,
    endpoint: Endpoint,
    provider: ProviderDefinition,
    secret: Arc<SecretMaterial>,
    client: Adapter,
    downstream: GenerationRepresentationContract,
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
