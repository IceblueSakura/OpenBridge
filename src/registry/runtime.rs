//! Immutable runtime entities produced by registry compilation.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use url::Url;

use crate::{
    config::{BootstrapConfig, HttpClientConfig, RuntimeLimits},
    core::{ApiProtocol, OperationKind, ReasoningOutput},
    provider::{CredentialKind, ProviderKind},
};

use super::{
    CanonicalModelTask, CanonicalTaskKind, IgnorableGenerationParameter, InputModality,
    ModelContextLength, OutputModality, PublicModel, ReasoningLevel, ReasoningSupport, RouteMode,
    UpstreamApiCapabilities,
};

/// Model metadata read by the request path after startup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInfo {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) tokenizer: Option<String>,
    pub(super) knowledge_cutoff: Option<String>,
    pub(super) task: CanonicalModelTask,
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
        self.task.context_length()
    }

    /// Returns the required canonical task identity without its task-specific payload.
    pub const fn task_kind(&self) -> CanonicalTaskKind {
        self.task.kind()
    }

    /// Returns the canonical task and its immutable task-specific payload.
    pub const fn task(&self) -> &CanonicalModelTask {
        &self.task
    }

    /// Returns confirmed input modalities; `None` does not mean explicitly unsupported.
    pub fn input_modalities(&self) -> Option<&[InputModality]> {
        self.task.input_modalities()
    }

    /// Returns confirmed output modalities; `None` does not mean explicitly unsupported.
    pub fn output_modalities(&self) -> Option<&[OutputModality]> {
        self.task.output_modalities()
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
        self.task.supported_parameters()
    }

    /// Returns the effective canonical reasoning support projection.
    pub const fn reasoning_support(&self) -> ReasoningSupport {
        self.task.reasoning_support()
    }

    /// Returns the effective reasoning levels.
    pub fn reasoning_levels(&self) -> &[ReasoningLevel] {
        self.task.reasoning_levels()
    }
}

/// Immutable registry snapshot read by the request path after startup.
#[derive(Debug)]
pub struct RuntimeRegistry {
    pub(super) version: RegistryVersion,
    pub(super) bootstrap: BootstrapConfig,
    pub(super) models: BTreeMap<String, ModelInfo>,
    pub(super) provider_instances: BTreeMap<String, Arc<ProviderInstance>>,
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

    /// Looks up a trusted Provider instance by its registry ID.
    pub fn provider_instance(&self, id: &str) -> Option<&ProviderInstance> {
        self.provider_instances.get(id).map(Arc::as_ref)
    }

    /// Enumerates all trusted Provider instance IDs.
    pub fn provider_instance_ids(&self) -> impl Iterator<Item = &str> {
        self.provider_instances.keys().map(String::as_str)
    }

    /// Looks up a validated credential pool by pool ID.
    pub fn credential_pool(&self, id: &str) -> Option<&CredentialPoolBinding> {
        self.credential_pools.get(id)
    }

    /// Enumerates all credential-pool IDs.
    pub fn credential_pool_ids(&self) -> impl Iterator<Item = &str> {
        self.credential_pools.keys().map(String::as_str)
    }

    /// Returns whether any enabled Target restricts the pool to at most one loaded member.
    pub fn credential_pool_requires_single_member(&self, pool_id: &str) -> bool {
        self.upstream_targets.values().any(|target| {
            target.enabled()
                && target.credential_pool_id() == pool_id
                && target
                    .upstream_apis
                    .values()
                    .any(UpstreamApi::requires_single_credential_member)
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

/// Validated trusted deployment of one compile-time Provider family.
#[derive(Debug)]
pub struct ProviderInstance {
    pub(super) id: String,
    pub(super) kind: ProviderKind,
    pub(super) endpoint_base: Url,
}

impl ProviderInstance {
    /// Returns the stable Provider instance ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the compile-time Provider family implemented by this instance.
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// Returns this instance's sole validated HTTPS base URL.
    pub fn endpoint_base(&self) -> &Url {
        &self.endpoint_base
    }
}

/// Upstream target that passed endpoint, credential, and model-reference validation.
#[derive(Debug)]
pub struct UpstreamTarget {
    pub(super) id: String,
    pub(super) provider_instance: Arc<ProviderInstance>,
    pub(super) credential_pool: String,
    pub(super) canonical_model_id: String,
    pub(super) provider_model_id: String,
    pub(super) quota_scope: Option<String>,
    pub(super) fault_domain: Option<String>,
    pub(super) request_timeout: Duration,
    pub(super) enabled: bool,
    pub(super) upstream_apis: BTreeMap<OperationKind, UpstreamApi>,
}

impl UpstreamTarget {
    /// Returns the target ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the Provider kind used by the target.
    pub fn kind(&self) -> ProviderKind {
        self.provider_instance.kind()
    }

    /// Returns the referenced Provider instance ID.
    pub fn provider_instance_id(&self) -> &str {
        self.provider_instance.id()
    }

    /// Returns the resolved trusted Provider instance.
    pub fn provider_instance(&self) -> &ProviderInstance {
        &self.provider_instance
    }

    /// Returns the credential-pool ID referenced by the target.
    pub fn credential_pool_id(&self) -> &str {
        &self.credential_pool
    }

    /// Returns the canonical designer/model identity referenced by the target.
    pub fn canonical_model_id(&self) -> &str {
        &self.canonical_model_id
    }

    /// Returns the trusted provider/model routing identity bound to the target.
    pub fn provider_model_id(&self) -> &str {
        &self.provider_model_id
    }

    /// Returns the validated endpoint base URL.
    pub fn endpoint_base(&self) -> &Url {
        self.provider_instance.endpoint_base()
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

    /// Looks up a resolved API by its typed operation.
    pub fn upstream_api(&self, operation: OperationKind) -> Option<&UpstreamApi> {
        self.upstream_apis.get(&operation)
    }

    /// Enumerates all typed Upstream API operations under the target.
    pub fn upstream_apis(&self) -> impl Iterator<Item = (OperationKind, &UpstreamApi)> {
        self.upstream_apis
            .iter()
            .map(|(operation, upstream_api)| (*operation, upstream_api))
    }
}

/// Upstream API with model rules resolved and applied.
#[derive(Debug)]
pub struct UpstreamApi {
    pub(super) model: ModelInfo,
    pub(super) upstream_model: String,
    pub(super) capabilities: UpstreamApiCapabilities,
    pub(super) streaming_policy: super::UpstreamStreamingPolicy,
    pub(super) reasoning_level_mappings: BTreeMap<ReasoningLevel, String>,
    pub(super) ignored_parameters: BTreeSet<IgnorableGenerationParameter>,
}

impl UpstreamApi {
    /// Returns the Upstream API's native operation.
    pub fn operation(&self) -> OperationKind {
        self.capabilities.operation()
    }

    /// Returns the generation protocol when this API can participate in the Protocol Bridge.
    pub fn api_protocol(&self) -> Option<ApiProtocol> {
        self.capabilities.api_protocol()
    }

    /// Returns model metadata after applying rules.
    pub fn model(&self) -> &ModelInfo {
        &self.model
    }

    /// Returns the actual model ID sent upstream.
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the API's protocol capabilities.
    pub fn capabilities(&self) -> UpstreamApiCapabilities {
        self.capabilities
    }

    /// Returns the reasoning output type declared by the API.
    pub fn reasoning_output(&self) -> ReasoningOutput {
        self.capabilities.reasoning_output()
    }

    /// Returns the trusted upstream streaming and non-streaming conversion policy.
    pub fn streaming_policy(&self) -> super::UpstreamStreamingPolicy {
        self.streaming_policy
    }

    /// Returns whether Provider state is bound to this concrete Upstream Target.
    pub fn is_target_bound(&self) -> bool {
        self.capabilities
            .responses()
            .is_some_and(|capabilities| capabilities.is_target_bound())
    }

    /// Returns whether this API accepts a Target-bound opaque response continuation.
    pub fn supports_previous_response_id(&self) -> bool {
        self.capabilities
            .responses()
            .is_some_and(|capabilities| capabilities.supports_previous_response_id())
    }

    /// Returns whether this API restricts its credential pool to at most one loaded member.
    pub fn requires_single_credential_member(&self) -> bool {
        self.capabilities
            .responses()
            .is_some_and(|capabilities| capabilities.requires_single_credential_member())
    }

    /// Returns the explicit wire mapping for a standard level on this Upstream API.
    pub fn reasoning_level_mapping(&self, level: ReasoningLevel) -> Option<&str> {
        self.reasoning_level_mappings
            .get(&level)
            .map(String::as_str)
    }

    /// Returns whether this Upstream API omits one accepted ordinary generation parameter.
    pub fn ignores_generation_parameter(&self, parameter: IgnorableGenerationParameter) -> bool {
        self.ignored_parameters.contains(&parameter)
    }

    /// Enumerates accepted ordinary generation parameters omitted from this API's egress body.
    pub(crate) fn ignored_generation_parameters(
        &self,
    ) -> impl Iterator<Item = IgnorableGenerationParameter> + '_ {
        self.ignored_parameters.iter().copied()
    }
}

/// Resolved Route binding.
#[derive(Debug)]
pub struct Route {
    pub(super) upstream_target: String,
    pub(super) upstream_operation: OperationKind,
    pub(super) downstream_operation: OperationKind,
    pub(super) mode: RouteMode,
}

impl Route {
    /// Returns the Upstream Target ID bound to the Route.
    pub fn upstream_target(&self) -> &str {
        &self.upstream_target
    }

    /// Returns the typed Upstream API operation bound to the Route.
    pub fn upstream_operation(&self) -> OperationKind {
        self.upstream_operation
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
