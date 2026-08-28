//! Purpose-bound API-key and OAuth2 manager materialization after registry validation.

use secrecy::SecretString;

use crate::{
    credential::{CredentialMetadata, CredentialSource, CredentialStoreBuilder},
    oauth2_credentials::{OAuth2CredentialManager, OAuth2CredentialManagerBuilder},
    provider::{CredentialKind, ProviderKind},
    registry::RuntimeRegistry,
};

use super::{
    ConfiguredCredentialSource, UpstreamCredentialConfigError, UpstreamCredentialConfiguration,
};

impl UpstreamCredentialConfiguration {
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

    /// Loads only the caller-requested OAuth2 pools into a guarded credential manager.
    ///
    /// The complete TOML binding set is validated first, but unselected OAuth2 auth files are not
    /// opened. This keeps an explicit probe scoped to the selected Provider and preserves the
    /// API-key loader's no-OAuth-file boundary.
    pub fn load_oauth2_for<'a>(
        mut self,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<OAuth2CredentialManager, UpstreamCredentialConfigError> {
        // Validate every configured binding before opening the selected OAuth2 auth file.
        let required = self.validate_for(registry, required_pool_ids)?;

        // Load only explicitly selected OAuth2 sources through the guarded lifecycle builder.
        let mut builder = OAuth2CredentialManagerBuilder::new();
        for pool_id in required {
            let pool = registry.credential_pool(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            if pool.kind() != CredentialKind::OAuth2BearerAccessToken {
                return Err(UpstreamCredentialConfigError::ApiKeyStoreRequired { id: pool_id });
            }
            let source = self.pools.remove(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                }
            })?;
            let ConfiguredCredentialSource::OAuth2AuthJsonFile(path) = source else {
                return Err(UpstreamCredentialConfigError::ApiKeyStoreRequired { id: pool_id });
            };
            builder
                .load_auth_json_file(pool.provider(), pool.id(), path)
                .map_err(UpstreamCredentialConfigError::OAuth2Credential)?;
        }
        Ok(builder.build())
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
                ConfiguredCredentialSource::ApiKeys(_) | ConfiguredCredentialSource::Inactive => {}
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
