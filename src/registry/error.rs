//! Registry-definition validation and compilation errors.

use thiserror::Error;

use crate::core::OperationKind;

/// Error returned when a compile-time registry definition is incomplete, inconsistent, or attempts to exceed its authority.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// The registry version is blank.
    #[error("registry version must not be blank")]
    BlankVersion,
    /// The credential-pool ID is blank.
    #[error("credential pool id must not be blank")]
    BlankCredentialPoolId,
    /// The Provider instance ID is blank.
    #[error("provider instance id must not be blank")]
    BlankProviderInstanceId,
    /// At least one active ChatGPT Target has no usable startup instructions.
    #[error("active ChatGPT targets require non-blank bootstrap chatgpt_instructions")]
    MissingChatGptInstructions,
    /// An entity collection contains a duplicate ID.
    #[error("duplicate {entity} id '{id}'")]
    DuplicateId {
        /// Entity type containing the conflict.
        entity: &'static str,
        /// Duplicated entity ID.
        id: String,
    },
    /// A definition references a missing entity.
    #[error("{entity} '{id}' references unknown {target} '{reference}'")]
    UnknownReference {
        /// Entity type making the reference.
        entity: &'static str,
        /// ID making the reference.
        id: String,
        /// Referenced entity type.
        target: &'static str,
        /// Unresolved reference value.
        reference: String,
    },
    /// A pool selects a credential type unsupported by the Provider.
    #[error("credential pool '{credential_pool}' uses a kind unsupported by its provider")]
    UnsupportedCredentialPoolKind {
        /// Incompatible pool ID.
        credential_pool: String,
    },
    /// The target and referenced pool belong to different Providers.
    #[error(
        "upstream target '{upstream_target}' and credential pool '{credential_pool}' use different providers"
    )]
    CredentialPoolProviderMismatch {
        /// Incompatible target ID.
        upstream_target: String,
        /// Incorrectly referenced pool ID.
        credential_pool: String,
    },
    /// The Provider instance endpoint is not an allowed HTTPS base URL.
    #[error("provider instance '{provider_instance}' uses an invalid base URL")]
    InvalidProviderBaseUrl {
        /// Provider instance ID with the invalid URL.
        provider_instance: String,
    },
    /// The target request timeout is zero.
    #[error("upstream target '{upstream_target}' request timeout must be greater than zero")]
    InvalidRequestTimeout {
        /// Target ID with the invalid timeout.
        upstream_target: String,
    },
    /// The target declares no Upstream API.
    #[error("upstream target '{upstream_target}' must contain at least one upstream API")]
    EmptyUpstreamTarget {
        /// Target ID with no Upstream API.
        upstream_target: String,
    },
    /// The target contains more than one Upstream API for the same operation.
    #[error(
        "upstream target '{upstream_target}' contains duplicate upstream operation '{upstream_operation}'"
    )]
    DuplicateUpstreamOperation {
        /// Target ID containing the conflict.
        upstream_target: String,
        /// Duplicated typed operation.
        upstream_operation: OperationKind,
    },
    /// The Upstream API upstream model ID is blank.
    #[error(
        "upstream operation '{upstream_operation}' on upstream target '{upstream_target}' upstream model must not be blank"
    )]
    BlankUpstreamModel {
        /// Owning target ID.
        upstream_target: String,
        /// Typed operation with the blank model ID.
        upstream_operation: OperationKind,
    },
    /// A registered Embeddings capability profile contains an invalid closed set, default, domain, or limit.
    #[error(
        "upstream operation '{upstream_operation}' on upstream target '{upstream_target}' has invalid Embeddings capabilities: {detail}"
    )]
    InvalidEmbeddingsCapabilities {
        /// Owning target ID.
        upstream_target: String,
        /// Typed operation with the invalid profile.
        upstream_operation: OperationKind,
        /// Stable validation detail without request or topology data.
        detail: &'static str,
    },
    /// An Upstream API streaming requirement or conversion mode conflicts with its typed operation.
    #[error(
        "upstream operation '{upstream_operation}' on upstream target '{upstream_target}' has invalid streaming policy: {detail}"
    )]
    InvalidUpstreamStreamingPolicy {
        /// Owning target ID.
        upstream_target: String,
        /// Typed operation with the invalid policy.
        upstream_operation: OperationKind,
        /// Stable validation detail without request or topology data.
        detail: &'static str,
    },
    /// An executable Upstream API profile is incompatible with its canonical Model task.
    #[error(
        "upstream operation '{upstream_operation}' on target '{upstream_target}' is incompatible with canonical model '{canonical_model}'"
    )]
    UpstreamApiModelTaskMismatch {
        /// Owning target ID.
        upstream_target: String,
        /// Typed operation with the incompatible executable profile.
        upstream_operation: OperationKind,
        /// Canonical Model ID whose task does not match the operation profile.
        canonical_model: String,
    },
    /// A required canonical-model string is blank.
    #[error("model '{model}' field '{field}' must not be blank")]
    BlankModelField {
        /// Invalid model ID.
        model: String,
        /// Blank field name.
        field: &'static str,
    },
    /// A canonical or Provider routing model identity does not use one non-empty namespace and model segment.
    #[error("model '{model}' field '{field}' must use the 'namespace/model' format")]
    InvalidNamespacedModelId {
        /// Invalid model identity.
        model: String,
        /// Registry field containing the invalid identity.
        field: &'static str,
    },
    /// A target's Provider routing identity does not match its Provider kind and canonical model basename.
    #[error(
        "upstream target '{upstream_target}' provider model '{provider_model}' must be '{expected}'"
    )]
    ProviderModelMismatch {
        /// Target containing the invalid routing identity.
        upstream_target: String,
        /// Configured Provider routing identity.
        provider_model: String,
        /// Expected Provider routing identity.
        expected: String,
    },
    /// The canonical model declares a zero context limit.
    #[error("model '{model}' context length '{limit}' must be greater than zero")]
    InvalidModelContextLength {
        /// Invalid model ID.
        model: String,
        /// Invalid limit field name.
        limit: &'static str,
    },
    /// The canonical model input or output limit exceeds its total context window.
    #[error("model '{model}' input or output limit exceeds its total context window")]
    InconsistentModelContextLength {
        /// Invalid model ID.
        model: String,
    },
    /// Explicit canonical-model task or modality facts are inconsistent.
    #[error("model '{model}' field '{field}' must be a non-empty unique capability set")]
    InconsistentModelCapabilities {
        /// Invalid model ID.
        model: String,
        /// Inconsistent capability field name.
        field: &'static str,
    },
    /// A canonical model parameter name does not use the restricted wire format.
    #[error("model '{model}' declares invalid supported parameter '{parameter}'")]
    InvalidSupportedParameter {
        /// Invalid model ID.
        model: String,
        /// Invalid parameter name.
        parameter: String,
    },
    /// The canonical model declares a parameter more than once.
    #[error("model '{model}' declares supported parameter '{parameter}' more than once")]
    DuplicateSupportedParameter {
        /// Model ID owning the duplicate parameter.
        model: String,
        /// Duplicated parameter name.
        parameter: String,
    },
    /// Upstream API model rules declare a zero limit.
    #[error("upstream API '{upstream_api}' model rule '{field}' must be greater than zero")]
    InvalidUpstreamApiModelRule {
        /// Upstream API identifier owning the rule.
        upstream_api: String,
        /// Invalid rule field.
        field: &'static str,
    },
    /// Upstream API model limits exceed the canonical model ceiling.
    #[error("upstream API '{upstream_api}' model rule '{field}' exceeds the model limit")]
    UpstreamApiModelLimitExceedsModel {
        /// Upstream API identifier owning the rule.
        upstream_api: String,
        /// Rule field that exceeds the ceiling.
        field: &'static str,
    },
    /// Upstream API model rules expand canonical model facts.
    #[error("upstream API '{upstream_api}' model rule '{field}' widens the model information")]
    UpstreamApiModelRuleWidensModel {
        /// Upstream API identifier owning the rule.
        upstream_api: String,
        /// Field being expanded.
        field: &'static str,
    },
    /// The Upstream API attempts to disable a parameter not declared by the model.
    #[error("upstream API '{upstream_api}' model rule disables undeclared parameter '{parameter}'")]
    UpstreamApiModelRuleDisablesUnknownParameter {
        /// Upstream API identifier owning the rule.
        upstream_api: String,
        /// Undeclared parameter being disabled.
        parameter: String,
    },
    /// The Upstream API attempts to ignore a parameter not declared by the model.
    #[error("upstream API '{upstream_api}' model rule ignores undeclared parameter '{parameter}'")]
    UpstreamApiModelRuleIgnoresUnknownParameter {
        /// Upstream API identifier owning the rule.
        upstream_api: String,
        /// Undeclared parameter being ignored.
        parameter: String,
    },
    /// Narrowed Upstream API reasoning configuration is inconsistent.
    #[error("upstream API '{upstream_api}' model rules are inconsistent: {detail}")]
    InconsistentUpstreamApiModelRules {
        /// Upstream API identifier owning the rule.
        upstream_api: String,
        /// Specific inconsistency reason.
        detail: &'static str,
    },
    /// The Upstream API declares capabilities beyond the Provider contract.
    #[error(
        "upstream operation '{upstream_operation}' on upstream target '{upstream_target}' enables capabilities unsupported by its adapter"
    )]
    CapabilityElevation {
        /// Owning target ID.
        upstream_target: String,
        /// Typed operation declaring excessive capabilities.
        upstream_operation: OperationKind,
    },
    /// A Native Route downstream operation differs from its Upstream API operation.
    #[error("native route '{route}' operation does not match its upstream API")]
    NativeRouteOperationMismatch {
        /// Route ID with the operation mismatch.
        route: String,
    },
    /// A Bridged Route does not connect the two distinct generation protocols.
    #[error("bridged route '{route}' must connect distinct generation protocol operations")]
    InvalidBridgedRouteOperations {
        /// Route ID whose operation direction has no conversion meaning.
        route: String,
    },
    /// The Public Model ID is not a safe single-segment URL resource identifier.
    #[error("public model '{public_model}' id is not a safe URL path segment")]
    InvalidPublicModelId {
        /// Invalid Public Model ID.
        public_model: String,
    },
    /// A Public Model public display field is blank.
    #[error("public model '{public_model}' field '{field}' must not be blank")]
    BlankPublicModelField {
        /// Invalid Public Model ID.
        public_model: String,
        /// Blank public field name.
        field: &'static str,
    },
    /// The Public Model has no valid stable creation time.
    #[error("public model '{public_model}' created timestamp must be greater than zero")]
    InvalidPublicModelCreated {
        /// Invalid Public Model ID.
        public_model: String,
    },
    /// Public Model lifecycle status and timestamps are inconsistent.
    #[error("public model '{public_model}' has inconsistent lifecycle timestamps")]
    InvalidPublicModelLifecycle {
        /// Invalid Public Model ID.
        public_model: String,
    },
    /// The Public Model references the same Route more than once.
    #[error("public model '{public_model}' contains duplicate route '{route}'")]
    DuplicatePublicModelRoute {
        /// Public Model name containing the conflict.
        public_model: String,
        /// Duplicated Route ID.
        route: String,
    },
    /// The Public Model has no Route.
    #[error("public model '{public_model}' must contain at least one route")]
    EmptyPublicModel {
        /// Public Model name with no Route.
        public_model: String,
    },
    /// A Public Model combines Route bindings that reference different canonical tasks.
    #[error("public model '{public_model}' combines incompatible canonical tasks")]
    PublicModelTaskMismatch {
        /// Public Model name containing the incompatible Route set.
        public_model: String,
    },
    /// A non-generation Public Model enables positive reasoning-level normalization.
    #[error(
        "public model '{public_model}' applies a reasoning-level policy to a non-generation task"
    )]
    PublicModelReasoningPolicyTaskMismatch {
        /// Public Model name containing the incompatible reasoning policy.
        public_model: String,
    },
    /// Same-task Route profiles have no valid common interface payload.
    #[error(
        "public model '{public_model}' has no common profile for downstream operation '{downstream_operation}'"
    )]
    PublicModelInterfaceProfileMismatch {
        /// Public Model name containing the incompatible interface profiles.
        public_model: String,
        /// Downstream operation whose profile intersection is empty.
        downstream_operation: OperationKind,
    },
    /// A Public Model contains more than one executable Embeddings candidate during the initial single-Route phase.
    #[error("public model '{public_model}' contains multiple executable Embeddings candidates")]
    MultipleEmbeddingsCandidates {
        /// Public Model name containing the unsupported candidate set.
        public_model: String,
    },
    /// The configured JSON response budget cannot contain one valid worst-case Embeddings result.
    #[error(
        "public model '{public_model}' cannot fit one Embeddings result within the JSON response budget"
    )]
    EmbeddingResponseBudgetTooSmall {
        /// Public Model whose maximum dimension and encoding cannot fit the runtime response limit.
        public_model: String,
    },
}
