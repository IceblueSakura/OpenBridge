//! TOML schema parsing and source-shape validation for upstream credentials.

use std::{collections::BTreeMap, fmt, path::PathBuf};

use serde::Deserialize;
use subtle::ConstantTimeEq;

use super::{UpstreamCredentialConfigError, UpstreamCredentialConfiguration};

const UPSTREAM_CREDENTIALS_SCHEMA_VERSION: u32 = 1;

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

        // Validate binding IDs and the mutually exclusive credential source selection for each entry.
        let mut pools = BTreeMap::new();
        for raw_pool in raw.credential_pools {
            let id = raw_pool.id.trim();
            if id.is_empty() {
                return Err(UpstreamCredentialConfigError::BlankPoolId);
            }
            let source = match (raw_pool.api_keys, raw_pool.auth_json_file) {
                (Some(_), Some(_)) => {
                    return Err(
                        UpstreamCredentialConfigError::ConflictingCredentialSources {
                            id: id.to_owned(),
                        },
                    );
                }
                (None, None) => ConfiguredCredentialSource::Inactive,
                (Some(api_keys), None) => {
                    validate_api_keys(id, &api_keys)?;
                    ConfiguredCredentialSource::ApiKeys(api_keys)
                }
                (None, Some(path)) => {
                    if path.as_os_str().to_string_lossy().trim().is_empty() {
                        return Err(UpstreamCredentialConfigError::BlankAuthJsonFile {
                            id: id.to_owned(),
                        });
                    }
                    ConfiguredCredentialSource::OAuth2AuthJsonFile(path)
                }
            };
            if pools.insert(id.to_owned(), source).is_some() {
                return Err(UpstreamCredentialConfigError::DuplicatePoolId { id: id.to_owned() });
            }
        }
        Ok(Self { pools })
    }
}

impl fmt::Debug for UpstreamCredentialConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key_pools = self
            .pools
            .values()
            .filter(|source| matches!(source, ConfiguredCredentialSource::ApiKeys(_)))
            .count();
        let oauth2_providers = self
            .pools
            .values()
            .filter(|source| matches!(source, ConfiguredCredentialSource::OAuth2AuthJsonFile(_)))
            .count();
        let inactive_pools = self
            .pools
            .values()
            .filter(|source| matches!(source, ConfiguredCredentialSource::Inactive))
            .count();
        formatter
            .debug_struct("UpstreamCredentialConfiguration")
            .field("credential_pools", &self.pools.len())
            .field("api_key_pools", &api_key_pools)
            .field("oauth2_providers", &oauth2_providers)
            .field("inactive_pools", &inactive_pools)
            .finish()
    }
}

pub(super) enum ConfiguredCredentialSource {
    ApiKeys(Vec<String>),
    OAuth2AuthJsonFile(PathBuf),
    Inactive,
}

/// Validates an API-key source for blank or duplicate secrets without exposing their values.
fn validate_api_keys(id: &str, api_keys: &[String]) -> Result<(), UpstreamCredentialConfigError> {
    // Reject blank members before comparing secrets for duplicates; an empty array intentionally disables the pool.
    if api_keys.iter().any(|key| key.trim().is_empty()) {
        return Err(UpstreamCredentialConfigError::BlankApiKey { id: id.to_owned() });
    }
    if contains_duplicate_secret(api_keys) {
        return Err(UpstreamCredentialConfigError::DuplicateApiKey { id: id.to_owned() });
    }
    Ok(())
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
    #[serde(default)]
    credential_pools: Vec<RawCredentialPool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCredentialPool {
    id: String,
    #[serde(default)]
    api_keys: Option<Vec<String>>,
    #[serde(default)]
    auth_json_file: Option<PathBuf>,
}
