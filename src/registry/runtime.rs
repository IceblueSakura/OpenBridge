//! Immutable runtime entities produced by registry compilation.

use std::{collections::BTreeMap, time::Duration};

use url::Url;

use crate::{
    config::{BootstrapConfig, HttpClientConfig, RuntimeLimits},
    core::{ApiProtocol, OperationKind, ReasoningOutput},
    provider::{CredentialKind, ProviderKind},
};

use super::{
    InputModality, ModelContextLength, ModelMode, OutputModality, PublicModel, ReasoningLevel,
    ReasoningSupport, RouteMode, StateAffinity, TransportKind, UpstreamApiCapabilities,
};

/// Model metadata read by the request path after startup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInfo {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) context_length: ModelContextLength,
    pub(super) mode: Option<ModelMode>,
    pub(super) input_modalities: Option<Vec<InputModality>>,
    pub(super) output_modalities: Option<Vec<OutputModality>>,
    pub(super) tokenizer: Option<String>,
    pub(super) knowledge_cutoff: Option<String>,
    pub(super) supported_parameters: Vec<String>,
    pub(super) reasoning: ReasoningSupport,
    pub(super) reasoning_levels: Vec<ReasoningLevel>,
}

impl ModelInfo {
    /// Returns the stable model ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the optional model description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns the effective context length.
    pub const fn context_length(&self) -> ModelContextLength {
        self.context_length
    }

    /// Returns the confirmed model task mode; `None` means it remains unknown at the definition layer.
    pub const fn mode(&self) -> Option<ModelMode> {
        self.mode
    }

    /// Returns confirmed input modalities; `None` does not mean explicitly unsupported.
    pub fn input_modalities(&self) -> Option<&[InputModality]> {
        self.input_modalities.as_deref()
    }

    /// Returns confirmed output modalities; `None` does not mean explicitly unsupported.
    pub fn output_modalities(&self) -> Option<&[OutputModality]> {
        self.output_modalities.as_deref()
    }

    /// Returns the tokenizer identifier, when the catalog confirms one.
    pub fn tokenizer(&self) -> Option<&str> {
        self.tokenizer.as_deref()
    }

    /// Returns the knowledge-cutoff date, when the catalog confirms one.
    pub fn knowledge_cutoff(&self) -> Option<&str> {
        self.knowledge_cutoff.as_deref()
    }

    /// Returns the effective supported parameters.
    pub fn supported_parameters(&self) -> &[String] {
        &self.supported_parameters
    }

    /// Returns the effective reasoning state.
    pub const fn reasoning(&self) -> ReasoningSupport {
        self.reasoning
    }

    /// Returns the effective reasoning levels.
    pub fn reasoning_levels(&self) -> &[ReasoningLevel] {
        &self.reasoning_levels
    }
}

/// Immutable registry snapshot read by the request path after startup.
#[derive(Debug)]
pub struct RuntimeRegistry {
    pub(super) version: RegistryVersion,
    pub(super) bootstrap: BootstrapConfig,
    pub(super) models: BTreeMap<String, ModelInfo>,
    pub(super) credential_pools: BTreeMap<String, CredentialPoolBinding>,
    pub(super) upstream_targets: BTreeMap<String, UpstreamTarget>,
    pub(super) routes: BTreeMap<String, Route>,
    pub(super) public_models: BTreeMap<String, PublicModel>,
}

impl RuntimeRegistry {
    /// Returns the compile-time registry version.
    pub fn version(&self) -> &RegistryVersion {
        &self.version
    }

    /// Returns the bootstrap loopback listen address.
    pub fn listen(&self) -> std::net::SocketAddr {
        self.bootstrap.listen()
    }

    /// Returns runtime resource limits.
    pub fn limits(&self) -> &RuntimeLimits {
        self.bootstrap.limits()
    }

    /// Returns the upstream HTTP client policy.
    pub fn http_client(&self) -> &HttpClientConfig {
        self.bootstrap.http_client()
    }

    /// Looks up model metadata by internal model ID.
    pub fn model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.get(id)
    }

    /// Looks up a validated credential pool by pool ID.
    pub fn credential_pool(&self, id: &str) -> Option<&CredentialPoolBinding> {
        self.credential_pools.get(id)
    }

    /// Enumerates all credential-pool IDs.
    pub fn credential_pool_ids(&self) -> impl Iterator<Item = &str> {
        self.credential_pools.keys().map(String::as_str)
    }

    /// Returns whether the pool serves a TargetBound Responses API with continuation enabled.
    pub fn credential_pool_requires_single_member(&self, pool_id: &str) -> bool {
        self.upstream_targets.values().any(|target| {
            target.enabled()
                && target.credential_pool_id() == pool_id
                && target.upstream_apis.values().any(|upstream_api| {
                    upstream_api.state_affinity() == StateAffinity::TargetBound
                        && matches!(
                            upstream_api.capabilities(),
                            UpstreamApiCapabilities::Responses(capabilities)
                                if capabilities.previous_response_id
                        )
                })
        })
    }

    /// Looks up a resolved target by internal target ID.
    pub fn upstream_target(&self, id: &str) -> Option<&UpstreamTarget> {
        self.upstream_targets.get(id)
    }

    /// Enumerates all internal target IDs.
    pub fn upstream_target_ids(&self) -> impl Iterator<Item = &str> {
        self.upstream_targets.keys().map(String::as_str)
    }

    /// Looks up a resolved Route by Route ID.
    pub fn route(&self, id: &str) -> Option<&Route> {
        self.routes.get(id)
    }

    /// Looks up a Public Model by its downstream name.
    pub fn public_model(&self, name: &str) -> Option<&PublicModel> {
        self.public_models
            .get(name)
            .filter(|model| model.is_available())
    }

    /// Enumerates Public Models exposed by the downstream `/v1/models` endpoint.
    pub fn public_models(&self) -> impl Iterator<Item = &PublicModel> {
        self.public_models
            .values()
            .filter(|model| model.is_available())
    }
}

/// Validated registry version identifier.
#[derive(Debug)]
pub struct RegistryVersion(pub(super) String);

impl RegistryVersion {
    /// Returns the version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resolved credential-pool binding.
#[derive(Debug)]
pub struct CredentialPoolBinding {
    pub(super) id: String,
    pub(super) provider: ProviderKind,
    pub(super) kind: CredentialKind,
}

impl CredentialPoolBinding {
    /// Returns the credential-pool ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the Provider allowed to consume this pool.
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// Returns the credential type.
    pub fn kind(&self) -> CredentialKind {
        self.kind
    }
}

/// Upstream target that passed endpoint, credential, and model-reference validation.
#[derive(Debug)]
pub struct UpstreamTarget {
    pub(super) id: String,
    pub(super) kind: ProviderKind,
    pub(super) credential_pool: String,
    pub(super) model_id: String,
    pub(super) endpoint_base: Url,
    pub(super) quota_scope: Option<String>,
    pub(super) fault_domain: Option<String>,
    pub(super) request_timeout: Duration,
    pub(super) enabled: bool,
    pub(super) upstream_apis: BTreeMap<String, UpstreamApi>,
}

impl UpstreamTarget {
    /// Returns the target ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the Provider kind used by the target.
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// Returns the credential-pool ID referenced by the target.
    pub fn credential_pool_id(&self) -> &str {
        &self.credential_pool
    }

    /// Returns the canonical model ID referenced by the target.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Returns the validated endpoint base URL.
    pub fn endpoint_base(&self) -> &Url {
        &self.endpoint_base
    }

    /// Returns the optional shared quota scope.
    pub fn quota_scope(&self) -> Option<&str> {
        self.quota_scope.as_deref()
    }

    /// Returns the optional fault-isolation domain.
    pub fn fault_domain(&self) -> Option<&str> {
        self.fault_domain.as_deref()
    }

    /// Returns the timeout for one upstream request.
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Returns whether new stateless requests may select the target.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Looks up a resolved API by Upstream API ID.
    pub fn upstream_api(&self, id: &str) -> Option<&UpstreamApi> {
        self.upstream_apis.get(id)
    }

    /// Finds an Upstream API by native protocol.
    pub fn upstream_api_for_protocol(&self, protocol: ApiProtocol) -> Option<&UpstreamApi> {
        self.upstream_apis
            .values()
            .find(|upstream_api| upstream_api.operation() == protocol.operation())
    }

    /// Enumerates all Upstream APIs and IDs under the target.
    pub fn upstream_apis(&self) -> impl Iterator<Item = (&str, &UpstreamApi)> {
        self.upstream_apis
            .iter()
            .map(|(id, upstream_api)| (id.as_str(), upstream_api))
    }
}

/// Upstream API with model rules resolved and applied.
#[derive(Debug)]
pub struct UpstreamApi {
    pub(super) operation: OperationKind,
    pub(super) model: ModelInfo,
    pub(super) upstream_model: String,
    pub(super) endpoint_profile: String,
    pub(super) transport: TransportKind,
    pub(super) capabilities: UpstreamApiCapabilities,
    pub(super) state_affinity: StateAffinity,
    pub(super) reasoning_level_mappings: BTreeMap<ReasoningLevel, String>,
}

impl UpstreamApi {
    /// Returns the Upstream API's native operation.
    pub fn operation(&self) -> OperationKind {
        self.operation
    }

    /// Returns the generation protocol when this API can participate in the Protocol Bridge.
    pub fn api_protocol(&self) -> Option<ApiProtocol> {
        self.operation.api_protocol()
    }

    /// Returns model metadata after applying rules.
    pub fn model(&self) -> &ModelInfo {
        &self.model
    }

    /// Returns the actual model ID sent upstream.
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the endpoint profile used for Provider identification.
    pub fn endpoint_profile(&self) -> &str {
        &self.endpoint_profile
    }

    /// Returns the transport profile in use.
    pub fn transport(&self) -> TransportKind {
        self.transport
    }

    /// Returns the API's protocol capabilities.
    pub fn capabilities(&self) -> UpstreamApiCapabilities {
        self.capabilities
    }

    /// Returns the reasoning output type declared by the API.
    pub fn reasoning_output(&self) -> ReasoningOutput {
        self.capabilities.reasoning_output()
    }

    /// Returns the ownership policy for continuation state.
    pub fn state_affinity(&self) -> StateAffinity {
        self.state_affinity
    }

    /// Returns the explicit wire mapping for a standard level on this Upstream API.
    pub fn reasoning_level_mapping(&self, level: ReasoningLevel) -> Option<&str> {
        self.reasoning_level_mappings
            .get(&level)
            .map(String::as_str)
    }
}

/// Resolved Route binding.
#[derive(Debug)]
pub struct Route {
    pub(super) upstream_target: String,
    pub(super) upstream_api: String,
    pub(super) downstream_operation: OperationKind,
    pub(super) mode: RouteMode,
}

impl Route {
    /// Returns the Upstream Target ID bound to the Route.
    pub fn upstream_target(&self) -> &str {
        &self.upstream_target
    }

    /// Returns the Upstream API ID bound to the Route.
    pub fn upstream_api(&self) -> &str {
        &self.upstream_api
    }

    /// Returns the downstream operation accepted by the Route.
    pub fn downstream_operation(&self) -> OperationKind {
        self.downstream_operation
    }

    /// Returns the generation protocol when the Route can participate in the Protocol Bridge.
    pub fn downstream_protocol(&self) -> Option<ApiProtocol> {
        self.downstream_operation.api_protocol()
    }

    /// Returns the Route handling mode.
    pub fn mode(&self) -> RouteMode {
        self.mode
    }
}
