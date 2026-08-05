//! Private upstream credential-pool configuration loaded at startup.
//!
//! The file stores only API keys for compile-time pool IDs; it cannot configure Providers,
//! credential kinds, endpoints, or routes. It is read once before network listening or probes,
//! and secrets are transferred to the immutable CredentialStore.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Path, PathBuf},
};

use secrecy::SecretString;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::{
    credential::{
        CredentialMetadata, CredentialSource, CredentialStoreBuilder, CredentialStoreError,
    },
    registry::RuntimeRegistry,
};

const UPSTREAM_CREDENTIALS_SCHEMA_VERSION: u32 = 1;

/// Parsed upstream credential configuration that passed document validation.
pub struct UpstreamCredentialConfiguration {
    pools: BTreeMap<String, Vec<String>>,
}

impl UpstreamCredentialConfiguration {
    /// Parses and validates upstream credential TOML.
    pub fn from_toml(document: &str) -> Result<Self, UpstreamCredentialConfigError> {
        // Parse the document and verify the schema version.
        let raw: RawUpstreamCredentials =
            toml::from_str(document).map_err(|_| UpstreamCredentialConfigError::Parse)?;
        if raw.schema_version != UPSTREAM_CREDENTIALS_SCHEMA_VERSION {
            return Err(UpstreamCredentialConfigError::UnsupportedSchema {
                actual: raw.schema_version,
            });
        }

        // Validate pool IDs and API-key collections, then index them by stable pool ID.
        let mut pools = BTreeMap::new();
        for raw_pool in raw.credential_pools {
            let id = raw_pool.id.trim();
            if id.is_empty() {
                return Err(UpstreamCredentialConfigError::BlankPoolId);
            }
            if raw_pool.api_keys.is_empty() {
                return Err(UpstreamCredentialConfigError::EmptyPool { id: id.to_owned() });
            }
            if raw_pool.api_keys.iter().any(|key| key.trim().is_empty()) {
                return Err(UpstreamCredentialConfigError::BlankApiKey { id: id.to_owned() });
            }
            if contains_duplicate_secret(&raw_pool.api_keys) {
                return Err(UpstreamCredentialConfigError::DuplicateApiKey { id: id.to_owned() });
            }
            if pools.insert(id.to_owned(), raw_pool.api_keys).is_some() {
                return Err(UpstreamCredentialConfigError::DuplicatePoolId { id: id.to_owned() });
            }
        }
        Ok(Self { pools })
    }

    /// Adds only the caller-requested pools to a new credential builder.
    pub fn into_builder_for<'a>(
        self,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item=&'a str>,
    ) -> Result<CredentialStoreBuilder, UpstreamCredentialConfigError> {
        let mut builder = CredentialStoreBuilder::new();
        self.load_into_for(&mut builder, registry, required_pool_ids)?;
        Ok(builder)
    }

    /// Validates the configuration against the compile-time registry and transfers requested pool secrets to the builder.
    pub fn load_into_for<'a>(
        mut self,
        builder: &mut CredentialStoreBuilder,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item=&'a str>,
    ) -> Result<(), UpstreamCredentialConfigError> {
        // Reject pools not registered in code so misspelled or stale secrets are not silently ignored.
        for configured_pool_id in self.pools.keys() {
            if registry.credential_pool(configured_pool_id).is_none() {
                return Err(UpstreamCredentialConfigError::UnknownPool {
                    id: configured_pool_id.clone(),
                });
            }
        }

        // Deduplicate and resolve the compile-time pools actually required by the caller.
        let required = required_pool_ids
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        for pool_id in &required {
            if registry.credential_pool(pool_id).is_none() {
                return Err(UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                });
            }
            if !self.pools.contains_key(pool_id) {
                return Err(UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                });
            }
        }

        // Transfer secrets only after validation succeeds so errors cannot leave a partial upstream pool.
        for pool_id in required {
            let pool = registry.credential_pool(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            let api_keys = self.pools.remove(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                }
            })?;

            // Generate stable member IDs in TOML array order and move secrets into the sole runtime store builder.
            for (index, api_key) in api_keys.into_iter().enumerate() {
                builder
                    .insert_upstream_member(
                        pool.provider(),
                        pool.id(),
                        format!("{}#{}", pool.id(), index + 1),
                        SecretString::from(api_key),
                        CredentialMetadata::upstream(
                            pool.kind(),
                            CredentialSource::UpstreamConfiguration,
                        ),
                    )
                    .map_err(UpstreamCredentialConfigError::Credential)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for UpstreamCredentialConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamCredentialConfiguration")
            .field("credential_pools", &self.pools.len())
            .finish()
    }
}

/// Checks for duplicate secrets within one pool using constant-time equality.
fn contains_duplicate_secret(secrets: &[String]) -> bool {
    secrets.iter().enumerate().any(|(index, candidate)| {
        secrets[..index].iter().any(|expected| {
            candidate.len() == expected.len()
                && bool::from(candidate.as_bytes().ct_eq(expected.as_bytes()))
        })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUpstreamCredentials {
    schema_version: u32,
    credential_pools: Vec<RawCredentialPool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCredentialPool {
    id: String,
    api_keys: Vec<String>,
}

/// Upstream credential TOML parsing, validation, or registry binding failed.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum UpstreamCredentialConfigError {
    /// The TOML document cannot be parsed as upstream credential configuration.
    #[error("invalid upstream credential configuration")]
    Parse,
    /// The document declares a schema version unsupported by this runtime.
    #[error("unsupported upstream credential configuration schema version {actual}")]
    UnsupportedSchema {
        /// Schema version declared by the document.
        actual: u32,
    },
    /// The credential pool ID is blank.
    #[error("upstream credential pool id must not be blank")]
    BlankPoolId,
    /// The same pool ID appears more than once.
    #[error("upstream credential pool '{id}' is configured more than once")]
    DuplicatePoolId {
        /// Duplicated pool ID.
        id: String,
    },
    /// The pool contains no API keys.
    #[error("upstream credential pool '{id}' must contain at least one API key")]
    EmptyPool {
        /// ID of the empty pool.
        id: String,
    },
    /// The pool contains a blank API key.
    #[error("upstream credential pool '{id}' contains a blank API key")]
    BlankApiKey {
        /// ID of the pool containing a blank API key.
        id: String,
    },
    /// The pool contains a duplicate API key.
    #[error("upstream credential pool '{id}' contains a duplicate API key")]
    DuplicateApiKey {
        /// ID of the pool containing a duplicate API key.
        id: String,
    },
    /// The TOML declares a pool absent from the compile-time registry.
    #[error("upstream credential configuration contains unknown pool '{id}'")]
    UnknownPool {
        /// Unregistered pool ID.
        id: String,
    },
    /// A compile-time pool required by the caller has no configured API key.
    #[error("upstream credential configuration is missing required pool '{id}'")]
    MissingPool {
        /// Missing pool ID.
        id: String,
    },
    /// An API key cannot be added to the purpose-bound credential store.
    #[error("upstream credential configuration could not populate the credential store")]
    Credential(#[source] CredentialStoreError),
}

/// Path to the upstream credential configuration file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialConfigPath(PathBuf);

impl UpstreamCredentialConfigPath {
    /// Creates an upstream credential configuration locator for the bootstrap-specified path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the upstream credential configuration file path.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Reads and parses the upstream credential configuration file.
    pub fn load(
        &self,
    ) -> Result<UpstreamCredentialConfiguration, UpstreamCredentialConfigFileError> {
        // Read the private configuration file while preserving path context.
        let document = fs::read_to_string(&self.0).map_err(|source| {
            UpstreamCredentialConfigFileError::Read {
                path: self.0.clone(),
                source,
            }
        })?;
        // Validate the contents and return a configuration object that does not expose secrets.
        UpstreamCredentialConfiguration::from_toml(&document)
            .map_err(UpstreamCredentialConfigFileError::Invalid)
    }
}

/// Reading or validating the upstream credential configuration file failed.
#[derive(Debug, Error)]
pub enum UpstreamCredentialConfigFileError {
    /// The upstream credential configuration file cannot be read.
    #[error("failed to read upstream credential configuration '{path}'")]
    Read {
        /// Path of the file that could not be read.
        path: PathBuf,
        #[source]
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// The file was read, but its contents failed upstream credential validation.
    #[error("upstream credential configuration validation failed")]
    Invalid(#[source] UpstreamCredentialConfigError),
}
