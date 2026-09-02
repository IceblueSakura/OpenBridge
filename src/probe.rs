//! Facade for explicit administrative upstream discovery and bounded unit API probes.
//!
//! Probes reuse the Upstream Target's trusted endpoint, credential, and compile-time adapter, but
//! do not use the downstream HTTP API or modify the code registry. `session` performs
//! trusted execution, `payload` owns fixed wire requests and response shapes, and
//! public reports provide redacted, point-in-time observations for service owners.

use http::StatusCode;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::core::ApiProtocol;

mod error;
mod payload;
mod session;

pub use error::{ProbeError, ProbeSelectionError};
pub use session::{
    probe_upstream_target, probe_upstream_target_with_oauth2, resolve_generation_probe_target,
};

/// Delivery mode exercised by one Generation probe case.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGenerationMode {
    /// Requests one bounded JSON response.
    NonStreaming,
    /// Requests one bounded SSE lifecycle.
    Streaming,
}

/// Fixed semantic capability exercised by one Generation probe case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeGenerationCapability {
    /// Baseline text generation without a structured-output request.
    Text,
    /// JSON Object response format.
    JsonObject,
    /// JSON Schema response format with `strict: false`.
    JsonSchema,
    /// JSON Schema response format with `strict: true`.
    JsonSchemaStrict,
    /// One fixed inline PNG image-input request.
    ImageInputInlinePng,
    /// One fixed function tool selected with `tool_choice=auto`.
    ToolAuto,
    /// One fixed function tool with calls explicitly disabled.
    ToolNone,
    /// One fixed function tool with any call required.
    ToolRequired,
    /// One fixed function tool selected by name.
    ToolNamed,
    /// One conflicting fixed function call constrained by a strict schema.
    ToolStrict,
    /// Two requested tools with parallel calls explicitly disabled.
    ToolParallelDisabled,
    /// Two requested tools with parallel calls explicitly enabled.
    ToolParallelEnabled,
}

impl ProbeGenerationCapability {
    /// Returns whether this capability exercises one fixed first-turn function-tool oracle.
    pub(crate) const fn is_tool_capability(self) -> bool {
        matches!(
            self,
            Self::ToolAuto
                | Self::ToolNone
                | Self::ToolRequired
                | Self::ToolNamed
                | Self::ToolStrict
                | Self::ToolParallelDisabled
                | Self::ToolParallelEnabled
        )
    }

    /// Returns the accuracy-oriented bounded output-token budget for every fixed oracle.
    pub(crate) const fn max_output_tokens(self) -> u32 {
        let _ = self;
        4_096
    }
}

/// Standard reasoning effort sent by one Generation probe case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeReasoningEffort {
    /// Omits the protocol reasoning field to establish a baseline.
    Omitted,
    /// Explicitly disables reasoning.
    None,
    /// Requests minimal reasoning.
    Minimal,
    /// Requests low reasoning.
    Low,
    /// Requests medium reasoning.
    Medium,
    /// Requests high reasoning.
    High,
    /// Requests extra-high reasoning.
    XHigh,
    /// Requests maximum reasoning.
    Max,
}

impl ProbeReasoningEffort {
    /// Returns the standard protocol value, or `None` for the omitted baseline.
    pub const fn as_wire(self) -> Option<&'static str> {
        match self {
            Self::Omitted => None,
            Self::None => Some("none"),
            Self::Minimal => Some("minimal"),
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
            Self::XHigh => Some("xhigh"),
            Self::Max => Some("max"),
        }
    }
}

/// One closed, unit-sized Generation capability case.
///
/// A case owns its reasoning semantics so callers cannot form meaningless reasoning × capability
/// combinations. Protocol and delivery remain explicit wire properties of the one request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGenerationCase {
    /// Baseline text with omitted reasoning controls.
    Text,
    /// Baseline text with reasoning explicitly disabled.
    ReasoningNone,
    /// Baseline text with minimal reasoning.
    ReasoningMinimal,
    /// Baseline text with low reasoning.
    ReasoningLow,
    /// Baseline text with medium reasoning.
    ReasoningMedium,
    /// Baseline text with high reasoning.
    ReasoningHigh,
    /// Baseline text with extra-high reasoning.
    ReasoningXHigh,
    /// Baseline text with maximum reasoning.
    ReasoningMax,
    /// JSON Object response format.
    JsonObject,
    /// Non-strict JSON Schema response format.
    JsonSchema,
    /// Strict JSON Schema response format.
    JsonSchemaStrict,
    /// Fixed inline PNG image input.
    ImageInputInlinePng,
    /// Function tools with automatic tool choice.
    ToolAuto,
    /// Function tools disabled by tool choice.
    ToolNone,
    /// At least one function tool required.
    ToolRequired,
    /// One named function tool required.
    ToolNamed,
    /// One strict-schema function tool.
    ToolStrict,
    /// Parallel tool calls explicitly disabled.
    ToolParallelDisabled,
    /// Parallel tool calls explicitly enabled.
    ToolParallelEnabled,
    /// Responses-only reasoning summary request with `summary: "auto"`.
    ReasoningSummary,
    /// Responses-only `include: ["reasoning.encrypted_content"]` request.
    IncludeEncryptedContent,
    /// Responses-only fixed `prompt_cache_key` hint request.
    PromptCacheKey,
}

impl ProbeGenerationCase {
    /// Parses one closed CLI case name.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "reasoning-none" => Some(Self::ReasoningNone),
            "reasoning-minimal" => Some(Self::ReasoningMinimal),
            "reasoning-low" => Some(Self::ReasoningLow),
            "reasoning-medium" => Some(Self::ReasoningMedium),
            "reasoning-high" => Some(Self::ReasoningHigh),
            "reasoning-xhigh" => Some(Self::ReasoningXHigh),
            "reasoning-max" => Some(Self::ReasoningMax),
            "json-object" => Some(Self::JsonObject),
            "json-schema" => Some(Self::JsonSchema),
            "json-schema-strict" => Some(Self::JsonSchemaStrict),
            "image-input-inline-png" => Some(Self::ImageInputInlinePng),
            "tool-auto" => Some(Self::ToolAuto),
            "tool-none" => Some(Self::ToolNone),
            "tool-required" => Some(Self::ToolRequired),
            "tool-named" => Some(Self::ToolNamed),
            "tool-strict" => Some(Self::ToolStrict),
            "tool-parallel-false" => Some(Self::ToolParallelDisabled),
            "tool-parallel-true" => Some(Self::ToolParallelEnabled),
            "reasoning-summary" => Some(Self::ReasoningSummary),
            "include-encrypted-content" => Some(Self::IncludeEncryptedContent),
            "prompt-cache-key" => Some(Self::PromptCacheKey),
            _ => None,
        }
    }

    /// Returns whether this case only exists on the Responses wire shape.
    pub(crate) const fn is_responses_only(self) -> bool {
        matches!(
            self,
            Self::ReasoningSummary | Self::IncludeEncryptedContent | Self::PromptCacheKey
        )
    }

    pub(crate) const fn reasoning_effort(self) -> ProbeReasoningEffort {
        match self {
            Self::ReasoningNone => ProbeReasoningEffort::None,
            Self::ReasoningMinimal => ProbeReasoningEffort::Minimal,
            Self::ReasoningLow => ProbeReasoningEffort::Low,
            Self::ReasoningMedium => ProbeReasoningEffort::Medium,
            Self::ReasoningHigh => ProbeReasoningEffort::High,
            Self::ReasoningXHigh => ProbeReasoningEffort::XHigh,
            Self::ReasoningMax => ProbeReasoningEffort::Max,
            // Both Responses-only differential cases pair a medium reasoning budget with the
            // single field under test; effort is fixed so the wire diff stays one-dimensional.
            Self::ReasoningSummary | Self::IncludeEncryptedContent => ProbeReasoningEffort::Medium,
            Self::Text
            | Self::JsonObject
            | Self::JsonSchema
            | Self::JsonSchemaStrict
            | Self::ImageInputInlinePng
            | Self::PromptCacheKey
            | Self::ToolAuto
            | Self::ToolNone
            | Self::ToolRequired
            | Self::ToolNamed
            | Self::ToolStrict
            | Self::ToolParallelDisabled
            | Self::ToolParallelEnabled => ProbeReasoningEffort::Omitted,
        }
    }

    pub(crate) const fn capability(self) -> ProbeGenerationCapability {
        match self {
            Self::Text
            | Self::ReasoningNone
            | Self::ReasoningMinimal
            | Self::ReasoningLow
            | Self::ReasoningMedium
            | Self::ReasoningHigh
            | Self::ReasoningXHigh
            | Self::ReasoningMax
            | Self::ReasoningSummary
            | Self::IncludeEncryptedContent
            | Self::PromptCacheKey => ProbeGenerationCapability::Text,
            Self::JsonObject => ProbeGenerationCapability::JsonObject,
            Self::JsonSchema => ProbeGenerationCapability::JsonSchema,
            Self::JsonSchemaStrict => ProbeGenerationCapability::JsonSchemaStrict,
            Self::ImageInputInlinePng => ProbeGenerationCapability::ImageInputInlinePng,
            Self::ToolAuto => ProbeGenerationCapability::ToolAuto,
            Self::ToolNone => ProbeGenerationCapability::ToolNone,
            Self::ToolRequired => ProbeGenerationCapability::ToolRequired,
            Self::ToolNamed => ProbeGenerationCapability::ToolNamed,
            Self::ToolStrict => ProbeGenerationCapability::ToolStrict,
            Self::ToolParallelDisabled => ProbeGenerationCapability::ToolParallelDisabled,
            Self::ToolParallelEnabled => ProbeGenerationCapability::ToolParallelEnabled,
        }
    }
}

/// One unit Generation case selection with optional admin-authored prompt/schema overrides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeGenerationSelection {
    /// OpenAI-compatible protocol used for the request.
    pub protocol: ProbeProtocol,
    /// JSON or SSE delivery mode.
    pub mode: ProbeGenerationMode,
    /// One closed unit capability case.
    pub case: ProbeGenerationCase,
    /// Optional admin-authored replacement for the case's fixed user prompt.
    pub custom_prompt: Option<String>,
    /// Optional admin-authored replacement for a JSON Schema case's response-format schema.
    pub custom_schema: Option<String>,
    /// Optional admin-authored replacement for the fixed response-format schema name.
    pub custom_schema_name: Option<String>,
}

/// Explicit administrative probe selection.
///
/// The CLI maps its `models` and `generation` subcommands into this library shape; internal callers
/// may also select the retained Embeddings smoke path directly.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProbeOptions {
    /// Whether to run the Provider's fixed model-list probe.
    pub list_models: bool,
    /// Optional single Generation request selection.
    pub generation: Option<ProbeGenerationSelection>,
    /// Whether to run the Embeddings Create probe.
    pub embeddings: bool,
    /// Optional model override used only by administrative discovery and Generation probes.
    pub upstream_model: Option<String>,
    /// Whether streaming probes may omit an upstream output-token limit.
    pub allow_unbounded_streaming_output: bool,
}

impl ProbeOptions {
    /// Returns whether no probe is selected.
    pub const fn is_empty(&self) -> bool {
        !self.list_models && self.generation.is_none() && !self.embeddings
    }

    /// Rejects malformed model IDs and inapplicable risk options before sensitive work.
    pub fn validate(&self) -> Result<(), ProbeSelectionError> {
        // Keep the override a literal bounded JSON value without trimming or normalization.
        if self.upstream_model.as_ref().is_some_and(|model| {
            model.is_empty()
                || model.len() > 256
                || model.trim() != model
                || model.chars().any(char::is_control)
        }) {
            return Err(ProbeSelectionError::InvalidUpstreamModel);
        }
        if self.upstream_model.is_some() && !self.list_models && self.generation.is_none() {
            return Err(ProbeSelectionError::UnusedUpstreamModel);
        }
        if self.allow_unbounded_streaming_output && self.generation.is_none() {
            return Err(ProbeSelectionError::UnusedUnboundedStreamingOutput);
        }
        if self.allow_unbounded_streaming_output
            && self
                .generation
                .as_ref()
                .is_some_and(|selection| selection.mode != ProbeGenerationMode::Streaming)
        {
            return Err(ProbeSelectionError::UnusedUnboundedStreamingOutput);
        }
        if let Some(selection) = self.generation.as_ref() {
            selection.validate_overrides()?;
        }
        Ok(())
    }
}

impl ProbeGenerationSelection {
    /// Validates admin-authored prompt/schema overrides against the selected closed case.
    fn validate_overrides(&self) -> Result<(), ProbeSelectionError> {
        // Responses-only differential cases cannot run on the Chat wire shape.
        if self.protocol == ProbeProtocol::ChatCompletions && self.case.is_responses_only() {
            return Err(ProbeSelectionError::ResponsesOnlyCase);
        }
        let custom_prompt = self.custom_prompt.as_deref().unwrap_or_default();
        let custom_schema = self.custom_schema.as_deref().unwrap_or_default();
        let custom_schema_name = self.custom_schema_name.as_deref().unwrap_or_default();
        if custom_prompt.is_empty() && custom_schema.is_empty() && custom_schema_name.is_empty() {
            return Ok(());
        }
        // Prompt overrides bind only to cases whose oracle tolerates a reworded request.
        if !custom_prompt.is_empty() {
            if self.case.capability().is_tool_capability() {
                return Err(ProbeSelectionError::UnsupportedPromptOverride);
            }
            if custom_prompt.len() > 4_096 {
                return Err(ProbeSelectionError::InvalidCustomPrompt);
            }
        }
        // A schema name is meaningful only together with the schema it names.
        if custom_schema.is_empty() && !custom_schema_name.is_empty() {
            return Err(ProbeSelectionError::InvalidCustomSchema);
        }
        if !custom_schema.is_empty() {
            if custom_schema.len() > 8_192 {
                return Err(ProbeSelectionError::InvalidCustomSchema);
            }
            let parsed_schema: Option<serde_json::Value> = serde_json::from_str(custom_schema).ok();
            if parsed_schema.is_none_or(|value| !value.is_object()) {
                return Err(ProbeSelectionError::InvalidCustomSchema);
            }
            if !matches!(
                self.case,
                ProbeGenerationCase::JsonSchema | ProbeGenerationCase::JsonSchemaStrict
            ) {
                return Err(ProbeSelectionError::UnsupportedSchemaOverride);
            }
        }
        if !custom_schema_name.is_empty()
            && (custom_schema_name.len() > 64
                || custom_schema_name.trim() != custom_schema_name
                || custom_schema_name
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control()))
        {
            return Err(ProbeSelectionError::InvalidCustomSchemaName);
        }
        Ok(())
    }
}

/// Point-in-time result of one bounded upstream exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    /// Upstream returned a recognizable success response for this exact request.
    Accepted,
    /// Upstream returned an HTTP or protocol-level rejection for this exact request.
    Rejected,
    /// The trusted Target/profile does not permit this operation or delivery case.
    Unsupported,
    /// Transport, limits, preparation, or malformed output prevented a conclusion.
    Inconclusive,
}

/// Safe failure stage emitted without retaining an upstream error body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeFailure {
    /// No model was available for a selected Generation case.
    ModelUnavailable,
    /// The Provider does not declare the selected operation path.
    OperationUnavailable,
    /// The selected Target API does not permit this upstream delivery mode.
    DeliveryUnavailable,
    /// The fixed request could not pass the Provider's trusted body preparation.
    RequestPreparation,
    /// The shared transport did not return an HTTP response.
    Transport,
    /// The bounded response body or SSE lifecycle exceeded a configured limit.
    ResponseLimit,
    /// A successful non-streaming response was not valid JSON.
    InvalidJson,
    /// A successful response did not match the minimum protocol envelope.
    UnexpectedResponse,
    /// A successful streaming response did not match the Provider SSE media policy.
    InvalidSseMediaType,
    /// SSE framing or event size validation failed.
    InvalidSse,
    /// The SSE stream ended without a recognized terminal event.
    MissingTerminal,
    /// The Provider emitted an explicit failed terminal event.
    UpstreamTerminalFailure,
}

/// Status, optional HTTP status, and a safe local failure stage for one probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProbeResult {
    /// Observation for this exact request; it is not a stability or enforcement claim.
    pub state: ProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// HTTP status returned by upstream; absent before a response is received.
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Safe failure stage without an upstream body, request body, or credential.
    pub failure: Option<ProbeFailure>,
}

impl ProbeResult {
    const fn accepted(status: StatusCode) -> Self {
        Self {
            state: ProbeStatus::Accepted,
            http_status: Some(status.as_u16()),
            failure: None,
        }
    }

    const fn from_http_status(status: StatusCode) -> Self {
        Self {
            state: ProbeStatus::Rejected,
            http_status: Some(status.as_u16()),
            failure: None,
        }
    }

    const fn unsupported(failure: ProbeFailure) -> Self {
        Self {
            state: ProbeStatus::Unsupported,
            http_status: None,
            failure: Some(failure),
        }
    }

    const fn rejected(status: StatusCode, failure: ProbeFailure) -> Self {
        Self {
            state: ProbeStatus::Rejected,
            http_status: Some(status.as_u16()),
            failure: Some(failure),
        }
    }

    const fn inconclusive(status: Option<StatusCode>, failure: ProbeFailure) -> Self {
        Self {
            state: ProbeStatus::Inconclusive,
            http_status: match status {
                Some(status) => Some(status.as_u16()),
                None => None,
            },
            failure: Some(failure),
        }
    }
}

/// Generation protocol selected for one probe case.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeProtocol {
    /// OpenAI Chat Completions wire.
    ChatCompletions,
    /// OpenAI Responses wire.
    Responses,
}

impl ProbeProtocol {
    pub(crate) const fn from_api(protocol: ApiProtocol) -> Self {
        match protocol {
            ApiProtocol::ChatCompletions => Self::ChatCompletions,
            ApiProtocol::Responses => Self::Responses,
        }
    }

    pub(crate) const fn as_api(self) -> ApiProtocol {
        match self {
            Self::ChatCompletions => ApiProtocol::ChatCompletions,
            Self::Responses => ApiProtocol::Responses,
        }
    }
}

/// Recognized terminal shape for one successful Generation response.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeTerminal {
    /// One valid non-streaming JSON envelope.
    NonStreaming,
    /// Chat Completions emitted `[DONE]`.
    ChatDone,
    /// Responses emitted `response.completed`.
    ResponsesCompleted,
    /// Responses emitted `response.incomplete`.
    ResponsesIncomplete,
    /// Responses emitted `response.failed`.
    ResponsesFailed,
}

/// Standard numeric token usage extracted without retaining generated text or raw bodies.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProbeTokenUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Prompt/input tokens when reported as a non-negative integer.
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Completion/output tokens when reported as a non-negative integer.
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Reasoning tokens from the protocol-specific output-token detail object.
    pub reasoning_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Total tokens when reported as a non-negative integer.
    pub total_tokens: Option<u64>,
}

/// Bounded protocol metadata extracted without retaining generated text or raw bodies.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GenerationProbeEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Canonical response media type when safely parseable.
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Recognized JSON/SSE terminal shape.
    pub terminal: Option<ProbeTerminal>,
    /// Whether a standard usage object appeared.
    pub usage_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Standard numeric token fields extracted from JSON or the terminal SSE response.
    pub usage: Option<ProbeTokenUsage>,
    /// Whether a standard output-text field appeared; generated text is never retained.
    pub output_text_observed: bool,
    /// Whether a standard reasoning field or event appeared; reasoning text is never retained.
    pub reasoning_observed: bool,
    /// Whether a reasoning summary appeared (Responses `summary` parts or summary SSE events);
    /// summary text is never retained.
    pub reasoning_summary_observed: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// Ordered unique SSE event-type tokens, capped by the probe implementation.
    pub event_types: Vec<String>,
}

/// One unit Generation case before execution.
#[derive(Clone)]
pub(crate) struct GenerationCaseSelection {
    pub(crate) protocol: ApiProtocol,
    pub(crate) mode: ProbeGenerationMode,
    pub(crate) case: ProbeGenerationCase,
    /// Validated admin-authored prompt replacement, when provided.
    pub(crate) custom_prompt: Option<String>,
    /// Validated admin-authored schema replacement, when provided.
    pub(crate) custom_schema: Option<String>,
    /// Validated admin-authored schema name replacement, when provided.
    pub(crate) custom_schema_name: Option<String>,
}

impl GenerationCaseSelection {
    pub(crate) fn reasoning_effort(&self) -> ProbeReasoningEffort {
        self.case.reasoning_effort()
    }

    pub(crate) fn capability(&self) -> ProbeGenerationCapability {
        self.case.capability()
    }
}

/// One independent Generation observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerationProbeResult {
    /// Selected protocol.
    pub protocol: ProbeProtocol,
    /// Selected delivery mode.
    pub mode: ProbeGenerationMode,
    /// Selected fixed unit capability case, including any reasoning effort.
    pub case: ProbeGenerationCase,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Exact model sent upstream, or absent when no registered/default model was available.
    pub upstream_model: Option<String>,
    /// Wall-clock duration of request preparation and the bounded exchange.
    pub elapsed_ms: u64,
    /// Exchange outcome for only this case.
    pub outcome: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Bounded protocol evidence available after a recognizable success response.
    pub evidence: Option<GenerationProbeEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Semantic oracle result derived from transient bounded output text.
    pub capability_evidence: Option<ProbeCapabilityEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Fingerprint of an admin-authored prompt override; the override text is never retained.
    pub custom_prompt_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Fingerprint of an admin-authored schema override; the override text is never retained.
    pub custom_schema_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Admin-authored schema name when it differs from the fixed default.
    pub custom_schema_name: Option<String>,
}

/// Returns a bounded hex fingerprint (first 16 SHA-256 hex chars) of one admin-authored override.
fn override_fingerprint(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Semantic result for one fixed Generation capability case.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeCapabilityVerdict {
    /// The completed response satisfied the fixed capability oracle.
    Supported,
    /// The request completed but the returned output did not satisfy the oracle.
    NotHonored,
    /// A terminal/resource condition prevented a semantic conclusion.
    Inconclusive,
}

/// Bounded semantic evidence that never retains generated output text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProbeCapabilityEvidence {
    /// Conclusion for this exact fixed request and output.
    pub verdict: ProbeCapabilityVerdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether output parsed as a JSON object for a structured-output case.
    pub valid_json_object: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether output matched the fixed `{"probe":"ok"}` schema oracle.
    pub fixed_schema_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether output contained the fixed token rendered into the built-in image.
    pub fixed_image_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Number of first-turn function calls observed for a tool capability case.
    pub tool_call_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether observed function names matched the fixed case oracle.
    pub fixed_tool_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether observed function arguments matched the fixed schema oracle.
    pub fixed_arguments_match: Option<bool>,
}

/// Model-list observation from the Provider's fixed discovery endpoint.
#[derive(Debug, Serialize)]
pub struct ModelListProbeResult {
    /// Conclusion for the fixed model-list request itself.
    pub outcome: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the configured upstream model appears in the returned list.
    pub configured_model_listed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the explicit `--model` value appears in the returned list.
    pub requested_model_listed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Total number of model IDs extracted before report sampling.
    pub model_id_count: Option<usize>,
    /// Whether `model_ids` was truncated to the bounded report sample.
    pub model_ids_truncated: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// Bounded sample of model IDs extracted from the response.
    pub model_ids: Vec<String>,
}

/// Probe report for one Upstream Target. It contains no credential, request body, or upstream response body.
#[derive(Debug, Serialize)]
pub struct TargetProbeReport {
    /// Internal target ID probed.
    pub upstream_target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Explicit administrative model override, when supplied.
    pub requested_model: Option<String>,
    /// Whether streaming cases were allowed to omit the upstream output-token limit.
    pub allow_unbounded_streaming_output: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Provider's fixed model-list endpoint.
    pub list_models: Option<ModelListProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// One independent Generation observation.
    pub generation: Option<GenerationProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Embeddings Create probe.
    pub embeddings: Option<ProbeResult>,
}

#[cfg(test)]
mod tests;
