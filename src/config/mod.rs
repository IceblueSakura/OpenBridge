use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
};

use arc_swap::ArcSwap;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::{core::CapabilitySet, provider::ProviderKind};

const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid bootstrap configuration: {0}")]
    BootstrapParse(#[source] toml::de::Error),
    #[error("invalid route configuration: {0}")]
    RouteParse(#[source] toml::de::Error),
    #[error("unsupported {document} schema version {actual}")]
    UnsupportedSchema { document: &'static str, actual: u32 },
    #[error("listen address '{listen}' must be a valid loopback socket address")]
    NonLoopbackListen { listen: String },
    #[error("provider '{provider}' credential must use a supported secret reference")]
    InvalidSecretReference { provider: String },
    #[error("unknown provider kind '{kind}'")]
    UnknownProviderKind { kind: String },
    #[error("duplicate {entity} id '{id}'")]
    DuplicateId { entity: &'static str, id: String },
    #[error("{entity} '{id}' references unknown {target} '{reference}'")]
    UnknownReference {
        entity: &'static str,
        id: String,
        target: &'static str,
        reference: String,
    },
    #[error("deployment '{deployment}' uses an invalid base URL")]
    InvalidBaseUrl { deployment: String },
    #[error("deployment '{deployment}' origin '{origin}' is not allowlisted")]
    OriginNotAllowed { deployment: String, origin: String },
    #[error("deployment '{deployment}' uses unsupported endpoint profile '{profile}'")]
    UnsupportedEndpointProfile { deployment: String, profile: String },
    #[error("deployment '{deployment}' enables capabilities unsupported by its adapter")]
    CapabilityElevation { deployment: String },
    #[error("alias '{alias}' must contain at least one deployment candidate")]
    EmptyAlias { alias: String },
}

#[derive(Debug)]
pub struct RegistrySnapshot {
    version: ConfigVersion,
    listen: SocketAddr,
    providers: BTreeMap<String, ResolvedProvider>,
    deployments: BTreeMap<String, ResolvedDeployment>,
    aliases: BTreeMap<String, ResolvedAlias>,
}

impl RegistrySnapshot {
    pub fn version(&self) -> &ConfigVersion {
        &self.version
    }

    pub fn listen(&self) -> SocketAddr {
        self.listen
    }

    pub fn provider(&self, id: &str) -> Option<&ResolvedProvider> {
        self.providers.get(id)
    }

    pub fn deployment(&self, id: &str) -> Option<&ResolvedDeployment> {
        self.deployments.get(id)
    }

    pub fn alias(&self, name: &str) -> Option<&ResolvedAlias> {
        self.aliases.get(name)
    }
}

pub struct ConfigManager {
    current: ArcSwap<RegistrySnapshot>,
}

impl ConfigManager {
    pub fn new(initial: RegistrySnapshot) -> Self {
        Self {
            current: ArcSwap::from_pointee(initial),
        }
    }

    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        self.current.load_full()
    }

    pub fn reload(&self, bootstrap_toml: &str, routes_toml: &str) -> Result<(), ConfigError> {
        let next = load_registry(bootstrap_toml, routes_toml)?;
        self.current.store(Arc::new(next));
        Ok(())
    }
}

#[derive(Debug)]
pub struct ConfigVersion(String);

impl ConfigVersion {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct ResolvedProvider {
    kind: ProviderKind,
}

impl ResolvedProvider {
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }
}

#[derive(Debug)]
pub struct ResolvedDeployment {
    origin: Url,
    #[allow(dead_code)]
    capabilities: CapabilitySet,
}

impl ResolvedDeployment {
    pub fn origin(&self) -> &Url {
        &self.origin
    }
}

#[derive(Debug)]
pub struct ResolvedAlias {
    candidates: Vec<String>,
}

impl ResolvedAlias {
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }
}

pub fn load_registry(
    bootstrap_toml: &str,
    routes_toml: &str,
) -> Result<RegistrySnapshot, ConfigError> {
    let bootstrap: RawBootstrap =
        toml::from_str(bootstrap_toml).map_err(ConfigError::BootstrapParse)?;
    let routes: RawRoutes = toml::from_str(routes_toml).map_err(ConfigError::RouteParse)?;

    validate_schema("bootstrap", bootstrap.schema_version)?;
    validate_schema("routes", routes.schema_version)?;
    let listen = bootstrap
        .listen
        .parse::<SocketAddr>()
        .ok()
        .filter(|address| address.ip().is_loopback())
        .ok_or_else(|| ConfigError::NonLoopbackListen {
            listen: bootstrap.listen.clone(),
        })?;

    let allowed_origins = bootstrap
        .allowed_origins
        .iter()
        .map(|origin| {
            normalize_origin(origin).ok_or_else(|| ConfigError::InvalidBaseUrl {
                deployment: "bootstrap allowlist".to_owned(),
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;

    let mut providers = BTreeMap::new();
    for provider in routes.providers {
        let kind = ProviderKind::from_config(&provider.kind).ok_or_else(|| {
            ConfigError::UnknownProviderKind {
                kind: provider.kind.clone(),
            }
        })?;
        if providers
            .insert(provider.id.clone(), ResolvedProvider { kind })
            .is_some()
        {
            return Err(ConfigError::DuplicateId {
                entity: "provider",
                id: provider.id,
            });
        }
        if !is_supported_secret_reference(&provider.credential.secret_ref) {
            return Err(ConfigError::InvalidSecretReference {
                provider: provider.id,
            });
        }
    }

    let mut deployments = BTreeMap::new();
    for deployment in routes.deployments {
        let provider =
            providers
                .get(&deployment.provider)
                .ok_or_else(|| ConfigError::UnknownReference {
                    entity: "deployment",
                    id: deployment.id.clone(),
                    target: "provider",
                    reference: deployment.provider.clone(),
                })?;
        if !provider
            .kind
            .accepts_endpoint_profile(&deployment.endpoint_profile)
        {
            return Err(ConfigError::UnsupportedEndpointProfile {
                deployment: deployment.id,
                profile: deployment.endpoint_profile,
            });
        }
        let origin =
            normalize_origin(&deployment.base_url).ok_or_else(|| ConfigError::InvalidBaseUrl {
                deployment: deployment.id.clone(),
            })?;
        if !allowed_origins.contains(&origin) {
            return Err(ConfigError::OriginNotAllowed {
                deployment: deployment.id,
                origin: origin.to_string(),
            });
        }
        if !deployment
            .capabilities
            .is_subset_of(provider.kind.capabilities())
        {
            return Err(ConfigError::CapabilityElevation {
                deployment: deployment.id,
            });
        }
        let resolved = ResolvedDeployment {
            origin,
            capabilities: deployment.capabilities,
        };
        if deployments
            .insert(deployment.id.clone(), resolved)
            .is_some()
        {
            return Err(ConfigError::DuplicateId {
                entity: "deployment",
                id: deployment.id,
            });
        }
    }

    let mut aliases = BTreeMap::new();
    for alias in routes.aliases {
        if alias.candidates.is_empty() {
            return Err(ConfigError::EmptyAlias { alias: alias.name });
        }
        for candidate in &alias.candidates {
            if !deployments.contains_key(candidate) {
                return Err(ConfigError::UnknownReference {
                    entity: "alias",
                    id: alias.name,
                    target: "deployment",
                    reference: candidate.clone(),
                });
            }
        }
        if aliases
            .insert(
                alias.name.clone(),
                ResolvedAlias {
                    candidates: alias.candidates,
                },
            )
            .is_some()
        {
            return Err(ConfigError::DuplicateId {
                entity: "alias",
                id: alias.name,
            });
        }
    }

    Ok(RegistrySnapshot {
        version: ConfigVersion(routes.config_version),
        listen,
        providers,
        deployments,
        aliases,
    })
}

fn validate_schema(document: &'static str, actual: u32) -> Result<(), ConfigError> {
    if actual == CONFIG_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConfigError::UnsupportedSchema { document, actual })
    }
}

fn is_supported_secret_reference(value: &str) -> bool {
    ["env://", "vault://"].iter().any(|prefix| {
        value
            .strip_prefix(prefix)
            .is_some_and(|name| !name.is_empty() && !name.chars().any(char::is_whitespace))
    })
}

fn normalize_origin(value: &str) -> Option<Url> {
    let mut url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (url.path() != "/" && !url.path().is_empty())
    {
        return None;
    }
    url.set_path("/");
    Some(url)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBootstrap {
    schema_version: u32,
    #[allow(dead_code)]
    listen: String,
    allowed_origins: Vec<String>,
    #[allow(dead_code)]
    max_request_body_bytes: usize,
    #[allow(dead_code)]
    max_sse_event_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoutes {
    schema_version: u32,
    config_version: String,
    providers: Vec<RawProvider>,
    deployments: Vec<RawDeployment>,
    aliases: Vec<RawAlias>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProvider {
    id: String,
    kind: String,
    credential: RawCredential,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCredential {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    kind: String,
    #[allow(dead_code)]
    secret_ref: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeployment {
    id: String,
    provider: String,
    #[allow(dead_code)]
    upstream_model: String,
    endpoint_profile: String,
    base_url: String,
    #[allow(dead_code)]
    request_timeout_ms: u64,
    capabilities: CapabilitySet,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAlias {
    name: String,
    candidates: Vec<String>,
}
