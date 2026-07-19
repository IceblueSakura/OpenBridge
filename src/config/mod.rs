use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use arc_swap::ArcSwap;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::{
    core::CapabilitySet,
    provider::{CredentialKind, ProviderKind},
};

const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid bootstrap configuration")]
    BootstrapParse,
    #[error("invalid route configuration")]
    RouteParse,
    #[error("bootstrap policy cannot change during route reload")]
    BootstrapPolicyChanged,
    #[error("unsupported {document} schema version {actual}")]
    UnsupportedSchema { document: &'static str, actual: u32 },
    #[error("listen address '{listen}' must be a valid loopback socket address")]
    NonLoopbackListen { listen: String },
    #[error("provider '{provider}' credential must use a supported secret reference")]
    InvalidSecretReference { provider: String },
    #[error("provider '{provider}' uses unsupported credential kind '{kind}'")]
    UnsupportedCredentialKind { provider: String, kind: String },
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
    #[error("deployment '{deployment}' request timeout must be greater than zero")]
    InvalidRequestTimeout { deployment: String },
    #[error("deployment '{deployment}' enables capabilities unsupported by its adapter")]
    CapabilityElevation { deployment: String },
    #[error("runtime limit '{name}' must be greater than zero")]
    InvalidLimit { name: &'static str },
    #[error("alias '{alias}' contains duplicate deployment candidate '{candidate}'")]
    DuplicateAliasCandidate { alias: String, candidate: String },
    #[error("alias '{alias}' must contain at least one deployment candidate")]
    EmptyAlias { alias: String },
}

#[derive(Debug)]
pub struct RegistrySnapshot {
    version: ConfigVersion,
    listen: SocketAddr,
    allowed_origins: BTreeSet<Url>,
    limits: RuntimeLimits,
    upstream_policy: UpstreamPolicy,
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

    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    pub fn upstream_policy(&self) -> &UpstreamPolicy {
        &self.upstream_policy
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

    fn has_same_bootstrap_policy(&self, other: &Self) -> bool {
        self.listen == other.listen
            && self.allowed_origins == other.allowed_origins
            && self.limits == other.limits
            && self.upstream_policy == other.upstream_policy
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
        if !self.snapshot().has_same_bootstrap_policy(&next) {
            return Err(ConfigError::BootstrapPolicyChanged);
        }
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
    credential: ResolvedCredential,
}

impl ResolvedProvider {
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn credential(&self) -> &ResolvedCredential {
        &self.credential
    }
}

#[derive(Debug)]
pub struct ResolvedCredential {
    id: String,
    kind: CredentialKind,
    secret_reference: SecretReference,
}

impl ResolvedCredential {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub fn secret_reference(&self) -> &SecretReference {
        &self.secret_reference
    }
}

#[derive(Debug)]
pub struct SecretReference {
    locator: String,
}

impl SecretReference {
    pub fn scheme(&self) -> &'static str {
        "env"
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeLimits {
    max_request_body_bytes: usize,
    max_sse_event_bytes: usize,
}

impl RuntimeLimits {
    pub fn max_request_body_bytes(&self) -> usize {
        self.max_request_body_bytes
    }

    pub fn max_sse_event_bytes(&self) -> usize {
        self.max_sse_event_bytes
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct UpstreamPolicy {
    connect_timeout: Duration,
    pool_idle_timeout: Duration,
    pool_max_idle_per_host: usize,
}

impl UpstreamPolicy {
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    pub fn pool_idle_timeout(&self) -> Duration {
        self.pool_idle_timeout
    }

    pub fn pool_max_idle_per_host(&self) -> usize {
        self.pool_max_idle_per_host
    }
}

#[derive(Debug)]
pub struct ResolvedDeployment {
    provider_id: String,
    upstream_model: String,
    endpoint_profile: String,
    origin: Url,
    request_timeout: Duration,
    capabilities: CapabilitySet,
}

impl ResolvedDeployment {
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub fn endpoint_profile(&self) -> &str {
        &self.endpoint_profile
    }

    pub fn origin(&self) -> &Url {
        &self.origin
    }

    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
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
        toml::from_str(bootstrap_toml).map_err(|_| ConfigError::BootstrapParse)?;
    let routes: RawRoutes = toml::from_str(routes_toml).map_err(|_| ConfigError::RouteParse)?;

    validate_schema("bootstrap", bootstrap.schema_version)?;
    validate_schema("routes", routes.schema_version)?;
    validate_nonzero_limit("max_request_body_bytes", bootstrap.max_request_body_bytes)?;
    validate_nonzero_limit("max_sse_event_bytes", bootstrap.max_sse_event_bytes)?;
    validate_nonzero_millis(
        "upstream_connect_timeout_ms",
        bootstrap.upstream_connect_timeout_ms,
    )?;
    validate_nonzero_millis(
        "upstream_pool_idle_timeout_ms",
        bootstrap.upstream_pool_idle_timeout_ms,
    )?;
    validate_nonzero_limit(
        "upstream_pool_max_idle_per_host",
        bootstrap.upstream_pool_max_idle_per_host,
    )?;
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
    let mut credential_ids = BTreeSet::new();
    for provider in routes.providers {
        let kind = ProviderKind::from_config(&provider.kind).ok_or_else(|| {
            ConfigError::UnknownProviderKind {
                kind: provider.kind.clone(),
            }
        })?;
        let credential_kind =
            CredentialKind::from_config(&provider.credential.kind).ok_or_else(|| {
                ConfigError::UnsupportedCredentialKind {
                    provider: provider.id.clone(),
                    kind: provider.credential.kind.clone(),
                }
            })?;
        if !kind.accepts_credential_kind(credential_kind) {
            return Err(ConfigError::UnsupportedCredentialKind {
                provider: provider.id,
                kind: provider.credential.kind,
            });
        }
        let secret_reference =
            parse_secret_reference(&provider.credential.secret_ref).ok_or_else(|| {
                ConfigError::InvalidSecretReference {
                    provider: provider.id.clone(),
                }
            })?;
        if !credential_ids.insert(provider.credential.id.clone()) {
            return Err(ConfigError::DuplicateId {
                entity: "credential",
                id: provider.credential.id,
            });
        }
        let resolved = ResolvedProvider {
            kind,
            credential: ResolvedCredential {
                id: provider.credential.id,
                kind: credential_kind,
                secret_reference,
            },
        };
        if providers.insert(provider.id.clone(), resolved).is_some() {
            return Err(ConfigError::DuplicateId {
                entity: "provider",
                id: provider.id,
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
        if deployment.request_timeout_ms == 0 {
            return Err(ConfigError::InvalidRequestTimeout {
                deployment: deployment.id,
            });
        }
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
            provider_id: deployment.provider,
            upstream_model: deployment.upstream_model,
            endpoint_profile: deployment.endpoint_profile,
            origin,
            request_timeout: Duration::from_millis(deployment.request_timeout_ms),
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
        let mut unique_candidates = BTreeSet::new();
        for candidate in &alias.candidates {
            if !unique_candidates.insert(candidate) {
                return Err(ConfigError::DuplicateAliasCandidate {
                    alias: alias.name,
                    candidate: candidate.clone(),
                });
            }
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
        allowed_origins,
        limits: RuntimeLimits {
            max_request_body_bytes: bootstrap.max_request_body_bytes,
            max_sse_event_bytes: bootstrap.max_sse_event_bytes,
        },
        upstream_policy: UpstreamPolicy {
            connect_timeout: Duration::from_millis(bootstrap.upstream_connect_timeout_ms),
            pool_idle_timeout: Duration::from_millis(bootstrap.upstream_pool_idle_timeout_ms),
            pool_max_idle_per_host: bootstrap.upstream_pool_max_idle_per_host,
        },
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

fn validate_nonzero_limit(name: &'static str, value: usize) -> Result<(), ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidLimit { name })
    } else {
        Ok(())
    }
}

fn validate_nonzero_millis(name: &'static str, value: u64) -> Result<(), ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidLimit { name })
    } else {
        Ok(())
    }
}

fn parse_secret_reference(value: &str) -> Option<SecretReference> {
    let locator = value.strip_prefix("env://")?;
    let mut characters = locator.chars();
    let first = characters.next()?;
    if !(first == '_' || first.is_ascii_alphabetic())
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(SecretReference {
        locator: locator.to_owned(),
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
    listen: String,
    allowed_origins: Vec<String>,
    max_request_body_bytes: usize,
    max_sse_event_bytes: usize,
    upstream_connect_timeout_ms: u64,
    upstream_pool_idle_timeout_ms: u64,
    upstream_pool_max_idle_per_host: usize,
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
    id: String,
    kind: String,
    secret_ref: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeployment {
    id: String,
    provider: String,
    upstream_model: String,
    endpoint_profile: String,
    base_url: String,
    request_timeout_ms: u64,
    capabilities: CapabilitySet,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAlias {
    name: String,
    candidates: Vec<String>,
}
