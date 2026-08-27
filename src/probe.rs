//! Facade for explicit administrative upstream discovery and bounded API probe matrices.
//!
//! Probes reuse the Upstream Target's trusted endpoint, credential, and compile-time adapter, but
//! do not use the downstream HTTP API or modify the code registry. `session` performs
//! trusted execution, `payload` owns fixed wire requests and response shapes, and
//! public reports provide redacted, point-in-time observations for service owners.

use http::StatusCode;
use serde::Serialize;

use crate::core::ApiProtocol;

mod error;
mod payload;
mod session;

pub use error::{ProbeError, ProbeSelectionError};
pub use session::{probe_upstream_target, probe_upstream_target_with_oauth2};

/// Delivery mode exercised by one Generation probe case.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGenerationMode {
    /// Requests one bounded JSON response.
    NonStreaming,
    /// Requests one bounded SSE lifecycle.
    Streaming,
}

/// Standard reasoning effort sent by one Generation probe case.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeReasoningEffort {
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
    #[serde(rename = "xhigh")]
    XHigh,
    /// Requests maximum reasoning.
    Max,
}

impl ProbeReasoningEffort {
    /// Stable baseline plus every standard OpenBridge reasoning level.
    pub const ALL: [Self; 8] = [
        Self::Omitted,
        Self::None,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::XHigh,
        Self::Max,
    ];

    /// Parses one CLI/wire label.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "omitted" => Some(Self::Omitted),
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

/// Explicit administrative probe selection. The CLI uses `all()` when no operation is supplied;
/// library callers may independently select discovery, protocols, delivery, and reasoning cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeOptions {
    /// Whether to run the Provider's fixed model-list probe.
    pub list_models: bool,
    /// Whether to run the Chat Completions text-request probe.
    pub chat: bool,
    /// Whether to run the Responses text-request probe.
    pub responses: bool,
    /// Whether to run the Embeddings Create probe.
    pub embeddings: bool,
    /// Optional model override used only by administrative discovery and Generation probes.
    pub upstream_model: Option<String>,
    /// Whether streaming probes may omit an upstream output-token limit.
    pub allow_unbounded_streaming_output: bool,
    /// Ordered Generation delivery cases.
    pub generation_modes: Vec<ProbeGenerationMode>,
    /// Ordered reasoning-effort cases.
    pub reasoning_efforts: Vec<ProbeReasoningEffort>,
}

impl Default for ProbeOptions {
    fn default() -> Self {
        Self {
            list_models: false,
            chat: false,
            responses: false,
            embeddings: false,
            upstream_model: None,
            allow_unbounded_streaming_output: false,
            generation_modes: vec![
                ProbeGenerationMode::NonStreaming,
                ProbeGenerationMode::Streaming,
            ],
            reasoning_efforts: vec![ProbeReasoningEffort::Omitted],
        }
    }
}

impl ProbeOptions {
    /// Selects every implemented probe.
    pub fn all() -> Self {
        Self {
            list_models: true,
            chat: true,
            responses: true,
            embeddings: true,
            ..Self::default()
        }
    }

    /// Returns whether no probe is selected.
    pub const fn is_empty(&self) -> bool {
        !self.list_models && !self.chat && !self.responses && !self.embeddings
    }

    /// Rejects malformed model IDs and ambiguous/empty matrix axes before sensitive work.
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
        if self.upstream_model.is_some() && !self.list_models && !self.chat && !self.responses {
            return Err(ProbeSelectionError::UnusedUpstreamModel);
        }

        // Require non-empty, duplicate-free axes only for selected Generation protocols.
        if self.chat || self.responses {
            if self.generation_modes.is_empty() {
                return Err(ProbeSelectionError::MissingGenerationMode);
            }
            if self.reasoning_efforts.is_empty() {
                return Err(ProbeSelectionError::MissingReasoningEffort);
            }
            if has_duplicates(&self.generation_modes) || has_duplicates(&self.reasoning_efforts) {
                return Err(ProbeSelectionError::DuplicateMatrixCase);
            }
        }
        Ok(())
    }
}

fn has_duplicates<T: Eq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// Ordered unique SSE event-type tokens, capped by the probe implementation.
    pub event_types: Vec<String>,
}

/// One independent Generation matrix observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerationProbeResult {
    /// Selected protocol.
    pub protocol: ProbeProtocol,
    /// Selected delivery mode.
    pub mode: ProbeGenerationMode,
    /// Selected reasoning differential.
    pub reasoning_effort: ProbeReasoningEffort,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// Independent Chat/Responses × delivery × reasoning observations.
    pub generation: Vec<GenerationProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Embeddings Create probe.
    pub embeddings: Option<ProbeResult>,
}

#[cfg(test)]
mod tests;
