//! Static definitions compiled into the registry.

use std::time::Duration;

use serde::Serialize;

use crate::{
    core::{
        ApiCapabilities, ApiProtocol, ChatCompletionsCapabilities, EmbeddingsCapabilities,
        GenerationCapabilities, OperationKind, ResponsesCapabilities,
    },
    provider::{CredentialKind, ProviderKind},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Evidence state for model reasoning capability.
pub enum ReasoningSupport {
    #[default]
    /// The configuration lacks enough evidence to determine reasoning support.
    Unknown,
    /// The model explicitly supports reasoning.
    Supported,
    /// The model explicitly does not support reasoning.
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
/// Reasoning levels supported by a model.
pub enum ReasoningLevel {
    /// Explicitly disables reasoning.
    None,
    /// Minimum reasoning level.
    Minimal,
    /// Low reasoning level.
    Low,
    /// Medium reasoning level.
    Medium,
    /// High reasoning level.
    High,
    /// Extra-high reasoning level.
    #[serde(rename = "xhigh")]
    XHigh,
    /// Maximum reasoning level.
    Max,
}

impl ReasoningLevel {
    /// Parses a protocol wire string into a catalog enum.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    /// Returns the wire string used by the standard downstream protocol.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Standard non-admission generation parameters that OpenBridge may omit from upstream egress.
///
/// This closed set intentionally excludes streaming, reasoning effort or switches, tools,
/// structured output, state, media, and output-budget fields whose omission would bypass a
/// capability or resource boundary.
pub enum IgnorableGenerationParameter {
    /// Frequency penalty applied during token sampling.
    FrequencyPenalty,
    /// Optional request for reasoning content in the response.
    IncludeReasoning,
    /// Chat token log-probability output switch.
    Logprobs,
    /// Number of Chat completion alternatives.
    N,
    /// Presence penalty applied during token sampling.
    PresencePenalty,
    /// Deterministic sampling seed hint.
    Seed,
    /// Sampling temperature.
    Temperature,
    /// Number of token log-probability alternatives.
    TopLogprobs,
    /// Nucleus-sampling probability threshold.
    TopP,
}

impl IgnorableGenerationParameter {
    /// Returns the standard top-level JSON field name for this parameter.
    pub const fn as_wire_name(self) -> &'static str {
        match self {
            Self::FrequencyPenalty => "frequency_penalty",
            Self::IncludeReasoning => "include_reasoning",
            Self::Logprobs => "logprobs",
            Self::N => "n",
            Self::PresencePenalty => "presence_penalty",
            Self::Seed => "seed",
            Self::Temperature => "temperature",
            Self::TopLogprobs => "top_logprobs",
            Self::TopP => "top_p",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Explicit mapping from a standard downstream reasoning level to an Upstream API wire level.
pub struct ReasoningLevelMapping {
    /// Standard downstream level declared by the Public Model.
    pub downstream: ReasoningLevel,
    /// Safe wire value accepted by the selected Upstream API.
    pub upstream: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
/// Independent model limits for total context, input, and output tokens.
pub struct ModelContextLength {
    /// Known combined input/output token limit; `None` means unknown.
    max_context_tokens: Option<u32>,
    /// Known maximum input token count; `None` means unknown. For OpenRouter-backed models this is
    /// populated from the model-level context length because the catalog does not publish a separate
    /// input-only limit.
    max_input_tokens: Option<u32>,
    /// Known maximum output token count; `None` means unknown.
    max_output_tokens: Option<u32>,
}

impl ModelContextLength {
    /// Creates independently optional total-context, input, and output limits.
    pub const fn new(
        max_context_tokens: Option<u32>,
        max_input_tokens: Option<u32>,
        max_output_tokens: Option<u32>,
    ) -> Self {
        Self {
            max_context_tokens,
            max_input_tokens,
            max_output_tokens,
        }
    }

    /// Returns the maximum combined input and output token count.
    pub const fn context_tokens(self) -> Option<u32> {
        self.max_context_tokens
    }

    /// Returns the maximum input token count.
    pub const fn input_tokens(self) -> Option<u32> {
        self.max_input_tokens
    }

    /// Returns the maximum output token count.
    pub const fn output_tokens(self) -> Option<u32> {
        self.max_output_tokens
    }
}

/// Task mode of a canonical Model.
///
/// OpenBridge currently registers only `Chat` models for Chat Completions/Responses generation.
/// This enum reserves a position for future model-information projection and is not used in registry
/// capability calculations.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelMode {
    /// Conversational text or multimodal generation model.
    Chat,
    /// Model that maps supported inputs to embedding vectors.
    Embedding,
}

/// Input modalities accepted by a canonical Model.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModality {
    /// Text input。
    Text,
    /// Image input。
    Image,
    /// Audio input。
    Audio,
    /// File input。
    File,
    /// Video input.
    Video,
}

/// Output modalities a canonical Model can generate.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputModality {
    /// Text output。
    Text,
    /// Image output。
    Image,
    /// Audio output。
    Audio,
    /// Numeric embedding-vector output.
    Embedding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Provider-independent canonical model facts.
pub struct ModelConfig {
    /// Stable model ID within the catalog.
    pub id: String,
    /// Model name shown to clients.
    pub name: String,
    /// Optional model description.
    pub description: Option<String>,
    /// Context length declared by the model.
    pub context_length: ModelContextLength,
    /// Confirmed model task mode; `None` means the definition has no evidence.
    pub mode: Option<ModelMode>,
    /// Confirmed input modalities; `None` means unknown, not an empty set or explicit rejection.
    pub input_modalities: Option<Vec<InputModality>>,
    /// Confirmed output modalities; `None` means unknown, not an empty set or explicit rejection.
    pub output_modalities: Option<Vec<OutputModality>>,
    /// Tokenizer identifier published by the model catalog; `None` means unknown.
    pub tokenizer: Option<String>,
    /// Knowledge-cutoff date published by the model catalog; `None` means unknown.
    pub knowledge_cutoff: Option<String>,
    /// OpenAI-compatible parameter names supported by the model.
    pub supported_parameters: Vec<String>,
    /// Model reasoning support state.
    pub reasoning: ReasoningSupport,
    /// Reasoning levels accepted by the model.
    pub reasoning_levels: Vec<ReasoningLevel>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// Model and ordinary-parameter rules applied by one Upstream API.
pub struct UpstreamApiModelRules {
    /// Context length the Upstream API may narrow further.
    pub context_length: ModelContextLength,
    /// Reasoning state the Upstream API may narrow further.
    pub reasoning: Option<ReasoningSupport>,
    /// Parameter names the Upstream API disables but cannot add.
    pub disabled_parameters: Vec<String>,
    /// Ordinary downstream parameters accepted by OpenBridge but omitted from upstream egress.
    pub ignored_parameters: Vec<IgnorableGenerationParameter>,
    /// Explicit mapping from standard downstream reasoning levels to this Upstream API's wire values.
    pub reasoning_level_mappings: Vec<ReasoningLevelMapping>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Credential-pool declaration shared by a Provider.
pub struct CredentialPoolConfig {
    /// Pool ID in the registry.
    pub id: String,
    /// Provider allowed to consume this pool.
    pub provider: ProviderKind,
    /// Credential type supported by the adapter.
    pub kind: CredentialKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One trusted deployment of a compile-time Provider family.
pub struct ProviderInstanceConfig {
    /// Stable Provider instance ID in the registry.
    pub id: String,
    /// Compile-time Provider family implemented by this instance.
    pub kind: ProviderKind,
    /// Sole trusted HTTPS base URL for this deployment.
    pub base_url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Upstream API capability configuration bound to a concrete protocol.
pub enum UpstreamApiCapabilities {
    /// Chat Completions endpoint capabilities.
    ChatCompletions(ChatCompletionsCapabilities),
    /// Responses endpoint capabilities.
    Responses(ResponsesCapabilities),
    /// Embeddings Create operation capabilities.
    Embeddings(EmbeddingsCapabilities),
}

impl UpstreamApiCapabilities {
    /// Returns the native operation represented by this capability configuration.
    pub const fn operation(self) -> OperationKind {
        match self {
            Self::ChatCompletions(_) => OperationKind::ChatCompletions,
            Self::Responses(_) => OperationKind::Responses,
            Self::Embeddings(_) => OperationKind::EmbeddingsCreate,
        }
    }

    /// Returns the generation protocol when this capability can participate in the Protocol Bridge.
    pub const fn api_protocol(self) -> Option<ApiProtocol> {
        self.operation().api_protocol()
    }

    /// Returns common protocol capabilities without Responses-specific state.
    pub(crate) const fn generation_capabilities(self) -> Option<GenerationCapabilities> {
        match self {
            Self::ChatCompletions(capabilities) => Some(capabilities.generation_capabilities()),
            Self::Responses(capabilities) => Some(capabilities.generation_capabilities()),
            Self::Embeddings(_) => None,
        }
    }

    /// Returns the complete capability set when this is an Embeddings configuration.
    pub const fn embeddings(self) -> Option<EmbeddingsCapabilities> {
        match self {
            Self::Embeddings(capabilities) => Some(capabilities),
            Self::ChatCompletions(_) | Self::Responses(_) => None,
        }
    }

    /// Returns the complete capability set when this is a Responses configuration.
    pub const fn responses(self) -> Option<ResponsesCapabilities> {
        match self {
            Self::ChatCompletions(_) => None,
            Self::Responses(capabilities) => Some(capabilities),
            Self::Embeddings(_) => None,
        }
    }

    /// Returns the reasoning output type declared by this Upstream API.
    pub const fn reasoning_output(self) -> crate::core::ReasoningOutput {
        match self {
            Self::ChatCompletions(capabilities) => capabilities.reasoning_output,
            Self::Responses(capabilities) => capabilities.reasoning_output,
            Self::Embeddings(_) => crate::core::ReasoningOutput::Unsupported,
        }
    }

    pub(super) fn is_subset_of(self, upper: ApiCapabilities) -> bool {
        match self {
            Self::ChatCompletions(capabilities) => {
                capabilities.is_subset_of(upper.chat_completions)
            }
            Self::Responses(capabilities) => capabilities.is_subset_of(upper.responses),
            Self::Embeddings(capabilities) => capabilities.is_subset_of(upper.embeddings),
        }
    }

    /// Returns whether this capability profile is statically enabled.
    pub(crate) const fn enabled(self) -> bool {
        match self {
            Self::ChatCompletions(capabilities) => capabilities.enabled,
            Self::Responses(capabilities) => capabilities.enabled,
            Self::Embeddings(capabilities) => capabilities.enabled,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Ownership scope for Provider-issued continuation state.
pub enum StateAffinity {
    /// The request carries no state that requires a fixed target.
    Unbound,
    /// State is bound to the current Upstream Target; cross-target fallback is forbidden.
    TargetBound,
}

/// Conversion policy for a downstream non-streaming request when an Upstream API requires SSE.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonStreamingConversion {
    /// Rejects non-streaming use of the streaming-only Upstream API before egress.
    Disabled,
    /// Buffers and validates a complete Responses SSE lifecycle before returning JSON downstream.
    BufferResponsesSse,
}

/// Declares whether an Upstream API accepts optional streaming or requires `stream: true`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamStreamingPolicy {
    /// Preserves the downstream streaming mode because the Upstream API accepts both modes.
    Optional,
    /// Forces `stream: true` upstream and controls whether non-streaming delivery can be synthesized.
    Required {
        /// Trusted conversion policy for downstream requests that require one complete JSON body.
        non_streaming: NonStreamingConversion,
    },
}

impl UpstreamStreamingPolicy {
    /// Returns whether the Upstream API can satisfy a downstream non-streaming request.
    pub const fn supports_non_streaming(self) -> bool {
        matches!(
            self,
            Self::Optional
                | Self::Required {
                    non_streaming: NonStreamingConversion::BufferResponsesSse
                }
        )
    }

    /// Returns whether every upstream request must use streaming transport.
    pub const fn requires_streaming(self) -> bool {
        matches!(self, Self::Required { .. })
    }

    /// Returns whether a non-streaming request must buffer a typed Responses SSE lifecycle.
    pub const fn buffers_responses_sse(self) -> bool {
        matches!(
            self,
            Self::Required {
                non_streaming: NonStreamingConversion::BufferResponsesSse
            }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Native Upstream API exposed by a target.
pub struct UpstreamApiConfig {
    /// Actual model ID sent upstream.
    pub upstream_model: String,
    /// Upstream API-level narrowing rules for Model facts.
    pub model_rules: UpstreamApiModelRules,
    /// Single-protocol capability evidence.
    pub capabilities: UpstreamApiCapabilities,
    /// Upstream streaming requirement and optional downstream non-streaming conversion.
    pub streaming_policy: UpstreamStreamingPolicy,
    /// Continuation/state ownership policy.
    pub state_affinity: StateAffinity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Trusted upstream target eligible for Route selection.
pub struct UpstreamTargetConfig {
    /// Target ID in the registry.
    pub id: String,
    /// Referenced Provider instance ID.
    pub provider_instance: String,
    /// Canonical designer/model identity whose provider-independent facts are used by this target.
    pub canonical_model: String,
    /// Trusted provider/model identity used by the routing layer for this target.
    pub provider_model: String,
    /// Shared credential-pool ID referenced by the target.
    pub credential_pool: String,
    /// Optional explicit shared quota scope.
    pub quota_scope: Option<String>,
    /// Optional fault/cooldown domain.
    pub fault_domain: Option<String>,
    /// Timeout for one upstream request.
    pub request_timeout: Duration,
    /// Whether new stateless requests may select this target.
    pub enabled: bool,
    /// Protocol-level Native supplies provided by the target.
    pub upstream_apis: Vec<UpstreamApiConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Request handling mode for a Route.
pub enum RouteMode {
    /// Keeps downstream and upstream protocols natively identical.
    Native,
    /// Performs an explicit restricted conversion between the two OpenAI-compatible protocols.
    Bridged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Route binding a downstream protocol to an Upstream API.
pub struct RouteConfig {
    /// Route ID in the registry.
    pub id: String,
    /// Upstream Target ID referenced by the Route.
    pub upstream_target: String,
    /// Typed Upstream API operation referenced by the Route.
    pub upstream_operation: OperationKind,
    /// Downstream operation accepted by the Route.
    pub downstream_operation: OperationKind,
    /// Route handling mode.
    pub mode: RouteMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Model exposed downstream with ordered Route candidates.
pub struct PublicModelConfig {
    /// Stable model ID exposed downstream.
    pub id: String,
    /// Stable Unix seconds when the Public Model contract was first created.
    pub created: u64,
    /// Name shown to clients.
    pub display_name: String,
    /// Optional description shown to clients.
    pub description: Option<String>,
    /// Static Public Model lifecycle.
    pub lifecycle: ModelLifecycle,
    /// Complete Route IDs ordered by priority.
    pub routes: Vec<String>,
}

/// Public Model lifecycle status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLifecycleStatus {
    /// The model accepts new requests.
    Active,
    /// The model remains callable, but callers should migrate.
    Deprecated,
    /// The model no longer accepts requests.
    Retired,
}

/// Static Public Model lifecycle information.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelLifecycle {
    /// Current lifecycle status.
    pub status: ModelLifecycleStatus,
    /// Optional Unix seconds when deprecation began.
    pub deprecated_at: Option<u64>,
    /// Optional Unix seconds when retirement began.
    pub retired_at: Option<u64>,
}

impl ModelLifecycle {
    /// Creates an active lifecycle with no deprecation or retirement time.
    pub const fn active() -> Self {
        Self {
            status: ModelLifecycleStatus::Active,
            deprecated_at: None,
            retired_at: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Complete definition required to compile the registry at startup.
pub struct RegistryConfig {
    /// Registry version used for reporting and audit.
    pub version: String,
    /// Complete model definitions.
    pub models: Vec<ModelConfig>,
    /// Complete trusted Provider instance definitions.
    pub provider_instances: Vec<ProviderInstanceConfig>,
    /// Complete credential-pool definitions.
    pub credential_pools: Vec<CredentialPoolConfig>,
    /// Complete Upstream Target definitions.
    pub upstream_targets: Vec<UpstreamTargetConfig>,
    /// Complete Route definitions.
    pub routes: Vec<RouteConfig>,
    /// Complete Public Model definitions.
    pub public_models: Vec<PublicModelConfig>,
}
