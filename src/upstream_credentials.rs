//! Private upstream credential-pool configuration loaded at startup.
//!
//! Each compile-time credential binding selects either ordered API keys or one OAuth2 auth-file
//! locator. The file cannot configure Providers, credential kinds, endpoints, or routes. It is
//! read once before network listening or probes. API keys become immutable snapshots; an OAuth2
//! locator becomes a guarded lifecycle target whose document may be refreshed transactionally.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
};

use secrecy::SecretString;
use serde::Deserialize;
use subtle::ConstantTimeEq;

use crate::{
    credential::{CredentialMetadata, CredentialSource, CredentialStoreBuilder},
    oauth2_credentials::{
        OAuth2CredentialManager, OAuth2CredentialManagerBuilder, OAuth2LoginTarget,
    },
    provider::CredentialKind,
    provider::ProviderKind,
    registry::RuntimeRegistry,
};

mod error;

pub use error::{UpstreamCredentialConfigError, UpstreamCredentialConfigFileError};

const UPSTREAM_CREDENTIALS_SCHEMA_VERSION: u32 = 1;

/// Parsed upstream credential configuration that passed document validation.
pub struct UpstreamCredentialConfiguration {
    pools: BTreeMap<String, ConfiguredCredentialSource>,
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

        // Validate binding IDs and the mutually exclusive credential source for each entry.
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
                (None, None) => {
                    return Err(UpstreamCredentialConfigError::MissingCredentialSource {
                        id: id.to_owned(),
                    });
                }
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

    /// Resolves one configured OAuth2 destination without reading or exposing its auth file.
    pub fn oauth2_login_target_for(
        &self,
        registry: &RuntimeRegistry,
        provider: ProviderKind,
    ) -> Result<OAuth2LoginTarget, UpstreamCredentialConfigError> {
        // Validate every configured binding before selecting the requested Provider destination.
        self.validate_for(registry, std::iter::empty())?;

        // Bind the fixed Provider to its sole registered OAuth2 pool and private file locator.
        for (pool_id, source) in &self.pools {
            let pool = registry.credential_pool(pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            if pool.provider() != provider {
                continue;
            }
            let ConfiguredCredentialSource::OAuth2AuthJsonFile(path) = source else {
                continue;
            };
            return Ok(OAuth2LoginTarget::new(provider, pool.id(), path.clone()));
        }
        Err(UpstreamCredentialConfigError::MissingOAuth2Provider { provider })
    }

    /// Adds only the caller-requested API-key pools to a new credential builder.
    pub fn into_builder_for<'a>(
        mut self,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<CredentialStoreBuilder, UpstreamCredentialConfigError> {
        // Validate the complete TOML binding set without opening any OAuth2 auth-file locator.
        let required = self.validate_for(registry, required_pool_ids)?;
        for pool_id in &required {
            let pool = registry.credential_pool(pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            if pool.kind() != CredentialKind::ApiKey {
                return Err(UpstreamCredentialConfigError::OAuth2ManagerRequired {
                    id: pool_id.clone(),
                });
            }
        }

        // Transfer only explicitly requested API-key sources for the selected probe target.
        let mut builder = CredentialStoreBuilder::new();
        for pool_id in required {
            let pool = registry.credential_pool(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            let source = self.pools.remove(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                }
            })?;
            let ConfiguredCredentialSource::ApiKeys(api_keys) = source else {
                return Err(UpstreamCredentialConfigError::OAuth2ManagerRequired { id: pool_id });
            };
            insert_api_key_members(
                &mut builder,
                pool.provider(),
                pool.kind(),
                pool.id(),
                api_keys,
            )?;
        }
        Ok(builder)
    }

    /// Validates and freezes API-key and OAuth2 sources against the compile-time registry.
    pub fn load_into_for<'a>(
        self,
        builder: &mut CredentialStoreBuilder,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<OAuth2CredentialManager, UpstreamCredentialConfigError> {
        // Validate the complete binding set and resolve the bindings required by enabled targets.
        let required = self.validate_for(registry, required_pool_ids)?;

        // Transfer required API keys and every configured OAuth2 file only after validation succeeds.
        let mut oauth2_builder = OAuth2CredentialManagerBuilder::new();
        for (pool_id, source) in self.pools {
            let pool = registry.credential_pool(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            match source {
                ConfiguredCredentialSource::ApiKeys(api_keys) if required.contains(&pool_id) => {
                    insert_api_key_members(
                        builder,
                        pool.provider(),
                        pool.kind(),
                        pool.id(),
                        api_keys,
                    )?;
                }
                ConfiguredCredentialSource::ApiKeys(_) => {}
                ConfiguredCredentialSource::OAuth2AuthJsonFile(path) => {
                    // Load one complete Provider-owned OAuth2 bundle into the guarded lifecycle manager.
                    oauth2_builder
                        .load_auth_json_file(pool.provider(), pool.id(), path)
                        .map_err(UpstreamCredentialConfigError::OAuth2Credential)?;
                }
            }
        }
        Ok(oauth2_builder.build())
    }

    /// Validates every configured source and returns the deduplicated required binding IDs.
    fn validate_for<'a>(
        &self,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<BTreeSet<String>, UpstreamCredentialConfigError> {
        // Reject unknown bindings, source-kind mismatches, and duplicate OAuth2 Provider ownership.
        let mut oauth2_providers = Vec::new();
        for (configured_pool_id, source) in &self.pools {
            let pool = registry
                .credential_pool(configured_pool_id)
                .ok_or_else(|| UpstreamCredentialConfigError::UnknownPool {
                    id: configured_pool_id.clone(),
                })?;
            let source_matches_kind = matches!(
                (source, pool.kind()),
                (
                    ConfiguredCredentialSource::ApiKeys(_),
                    CredentialKind::ApiKey
                ) | (
                    ConfiguredCredentialSource::OAuth2AuthJsonFile(_),
                    CredentialKind::OAuth2BearerAccessToken
                )
            );
            if !source_matches_kind {
                return Err(
                    UpstreamCredentialConfigError::CredentialSourceKindMismatch {
                        id: configured_pool_id.clone(),
                    },
                );
            }
            if matches!(source, ConfiguredCredentialSource::OAuth2AuthJsonFile(_)) {
                if oauth2_providers.contains(&pool.provider()) {
                    return Err(UpstreamCredentialConfigError::DuplicateOAuth2Provider {
                        provider: pool.provider(),
                    });
                }
                oauth2_providers.push(pool.provider());
            }
        }

        // Deduplicate and require every binding selected by an enabled target or explicit probe.
        let required = required_pool_ids
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        for pool_id in &required {
            registry.credential_pool(pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            if !self.pools.contains_key(pool_id) {
                return Err(UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                });
            }
        }
        Ok(required)
    }

    /// Resolves relative OAuth2 locators against the private TOML document directory.
    fn resolve_auth_json_files(&mut self, directory: &Path) {
        // Resolve only new relative locators; API keys and absolute paths remain unchanged.
        for source in self.pools.values_mut() {
            let ConfiguredCredentialSource::OAuth2AuthJsonFile(path) = source else {
                continue;
            };
            if path.is_relative() {
                *path = directory.join(&*path);
            }
        }
    }
}

impl fmt::Debug for UpstreamCredentialConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key_pools = self
            .pools
            .values()
            .filter(|source| matches!(source, ConfiguredCredentialSource::ApiKeys(_)))
            .count();
        let oauth2_providers = self.pools.len() - api_key_pools;
        formatter
            .debug_struct("UpstreamCredentialConfiguration")
            .field("credential_pools", &self.pools.len())
            .field("api_key_pools", &api_key_pools)
            .field("oauth2_providers", &oauth2_providers)
            .finish()
    }
}

enum ConfiguredCredentialSource {
    ApiKeys(Vec<String>),
    OAuth2AuthJsonFile(PathBuf),
}

/// Moves one validated ordered API-key source into the immutable store builder.
fn insert_api_key_members(
    builder: &mut CredentialStoreBuilder,
    provider: ProviderKind,
    kind: CredentialKind,
    pool_id: &str,
    api_keys: Vec<String>,
) -> Result<(), UpstreamCredentialConfigError> {
    // Generate stable member IDs in TOML order and move every secret into the purpose-bound store.
    for (index, api_key) in api_keys.into_iter().enumerate() {
        builder
            .insert_upstream_member(
                provider,
                pool_id,
                format!("{pool_id}#{}", index + 1),
                SecretString::from(api_key),
                CredentialMetadata::upstream(kind, CredentialSource::UpstreamConfiguration),
            )
            .map_err(UpstreamCredentialConfigError::Credential)?;
    }
    Ok(())
}

/// Validates a non-empty, duplicate-free API-key source without exposing any value.
fn validate_api_keys(id: &str, api_keys: &[String]) -> Result<(), UpstreamCredentialConfigError> {
    // Validate member availability before comparing secrets for duplicates.
    if api_keys.is_empty() {
        return Err(UpstreamCredentialConfigError::EmptyPool { id: id.to_owned() });
    }
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
        // Validate the contents before resolving locators relative to this private document.
        let mut configuration = UpstreamCredentialConfiguration::from_toml(&document)
            .map_err(UpstreamCredentialConfigFileError::Invalid)?;
        let directory = self.0.parent().unwrap_or_else(|| Path::new("."));
        configuration.resolve_auth_json_files(directory);
        Ok(configuration)
    }
}
