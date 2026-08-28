//! Registry ownership and required-pool binding for validated credential sources.

use std::collections::BTreeSet;

use crate::{
    oauth2_credentials::OAuth2LoginTarget,
    provider::{CredentialKind, ProviderKind},
    registry::RuntimeRegistry,
};

use super::{
    ConfiguredCredentialSource, UpstreamCredentialConfigError, UpstreamCredentialConfiguration,
};

impl UpstreamCredentialConfiguration {
    /// Enumerates configured credential pools with a usable startup source.
    ///
    /// API-key pools are active only when at least one key is present. OAuth2 pools are active
    /// when their non-empty auth-file locator is configured; the locator may still resolve to a
    /// missing or empty OpenBridge-owned file and remain pending login.
    pub fn active_pool_ids(&self) -> impl Iterator<Item = &str> {
        self.pools.iter().filter_map(|(pool_id, source)| {
            let active = match source {
                ConfiguredCredentialSource::ApiKeys(api_keys) => !api_keys.is_empty(),
                ConfiguredCredentialSource::OAuth2AuthJsonFile(_) => true,
                ConfiguredCredentialSource::Inactive => false,
            };
            active.then_some(pool_id.as_str())
        })
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

    /// Validates every configured source and returns the deduplicated required binding IDs.
    pub(super) fn validate_for<'a>(
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
                (ConfiguredCredentialSource::Inactive, _)
                    | (
                        ConfiguredCredentialSource::ApiKeys(_),
                        CredentialKind::ApiKey
                    )
                    | (
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
}
