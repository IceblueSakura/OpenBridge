//! Credential store facade.
//!
//! Startup construction, immutable runtime lookup, secret material, and error contracts live in
//! focused children. This facade preserves the existing `credential::store` boundary without
//! exposing generic secret access.

mod builder;
mod error;
mod material;
mod runtime;

#[cfg(test)]
mod tests;

pub use builder::CredentialStoreBuilder;
pub use error::CredentialStoreError;
pub use material::UpstreamCredential;
pub use runtime::CredentialStore;
