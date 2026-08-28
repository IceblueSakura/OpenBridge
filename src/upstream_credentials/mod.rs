//! Private upstream credential-pool configuration loaded at startup.
//!
//! Parsing, registry binding, secret materialization, and private-file loading remain separate
//! startup-only leaves behind this stable facade.

use std::collections::BTreeMap;

mod binding;
mod document;
mod error;
mod materialize;
mod source;

use document::ConfiguredCredentialSource;
pub use error::{UpstreamCredentialConfigError, UpstreamCredentialConfigFileError};
pub use source::UpstreamCredentialConfigPath;

/// Parsed upstream credential configuration that passed document validation.
pub struct UpstreamCredentialConfiguration {
    pub(in crate::upstream_credentials) pools: BTreeMap<String, ConfiguredCredentialSource>,
}
