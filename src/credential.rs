//! Purpose-bound credential facade.
//!
//! Non-sensitive identities and metadata live in `types`; immutable secret ownership, startup
//! validation, and restricted egress views live in `store`. This facade preserves the existing
//! `crate::credential::*` paths without exposing a generic secret lookup API.

mod store;
mod types;

pub use store::{
    CredentialStore, CredentialStoreBuilder, CredentialStoreError, UpstreamCredential,
};
pub use types::{CredentialId, CredentialMetadata, CredentialSource, CredentialType};
