//! Facade for explicit administrative upstream capability probes.
//!
//! Probes reuse the Upstream Target's trusted endpoint, credential, and compile-time adapter, but
//! do not use the downstream HTTP API or modify the code registry. `session` performs
//! trusted execution, `payload` owns fixed wire requests and response shapes, and
//! public reports provide evidence for service owners updating capability configuration.

use http::StatusCode;
use serde::Serialize;

mod error;
mod payload;
mod session;

pub use error::ProbeError;
pub use session::{probe_upstream_target, probe_upstream_target_with_oauth2};

/// Explicit probe selection. The CLI uses `all()` when no selection is supplied;
/// library callers may run only the free `list_models` probe or validate one protocol.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProbeOptions {
    /// Whether to run the Provider's fixed model-list probe.
    pub list_models: bool,
    /// Whether to run the Chat Completions text-request probe.
    pub chat: bool,
    /// Whether to run the Responses text-request probe.
    pub responses: bool,
    /// Whether to run the function-call and result-replay probe.
    pub function_calling: bool,
}

impl ProbeOptions {
    /// Selects every implemented probe.
    pub const fn all() -> Self {
        Self {
            list_models: true,
            chat: true,
            responses: true,
            function_calling: true,
        }
    }

    /// Returns whether no probe is selected.
    pub const fn is_empty(self) -> bool {
        !self.list_models && !self.chat && !self.responses && !self.function_calling
    }
}

/// Conservative probe conclusion for one capability.
///
/// `unsupported` is used only when the endpoint explicitly does not exist (404/405/501).
/// Authentication, rate limits, network failures, and rejected request shapes remain
/// `unknown` so transient failures are not reported as static lack of support.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    /// The request matched the protocol shape expected by the probe.
    Supported,
    /// The endpoint explicitly returned a status indicating that the operation is unsupported.
    Unsupported,
    /// The request failed or the response shape is insufficient for a conclusion.
    Unknown,
}

/// Status and optional HTTP status for one probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProbeResult {
    /// Conservative conclusion for this probe.
    pub state: SupportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// HTTP status returned by upstream; absent before a response is received.
    pub http_status: Option<u16>,
}

impl ProbeResult {
    const fn supported(status: StatusCode) -> Self {
        Self {
            state: SupportStatus::Supported,
            http_status: Some(status.as_u16()),
        }
    }

    const fn from_http_status(status: StatusCode) -> Self {
        Self {
            state: if matches!(
                status,
                StatusCode::NOT_FOUND
                    | StatusCode::METHOD_NOT_ALLOWED
                    | StatusCode::NOT_IMPLEMENTED
            ) {
                SupportStatus::Unsupported
            } else {
                SupportStatus::Unknown
            },
            http_status: Some(status.as_u16()),
        }
    }

    const fn unknown(status: Option<StatusCode>) -> Self {
        Self {
            state: SupportStatus::Unknown,
            http_status: match status {
                Some(status) => Some(status.as_u16()),
                None => None,
            },
        }
    }
}

/// Model-list observation from the Provider's fixed discovery endpoint.
#[derive(Debug, Serialize)]
pub struct ModelListProbeResult {
    /// Conclusion for the fixed model-list request itself.
    pub outcome: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the configured upstream model appears in the returned list.
    pub configured_model_listed: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// Model IDs extracted from the response; the list may be empty or incomplete.
    pub model_ids: Vec<String>,
}

/// Observation from the function-calling probe and its tool-result replay.
#[derive(Debug, Serialize)]
pub struct ToolCallProbeResult {
    /// Conclusion for the initial function-call request.
    pub initial_call: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Conclusion for the request after replaying the tool result.
    pub result_replay: Option<ProbeResult>,
}

/// Probe report for one Upstream Target. It contains no credential, request body, or upstream response body.
#[derive(Debug, Serialize)]
pub struct TargetProbeReport {
    /// Internal target ID probed.
    pub upstream_target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Provider's fixed model-list endpoint.
    pub list_models: Option<ModelListProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Chat Completions text probe.
    pub chat: Option<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Responses text probe.
    pub responses: Option<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Chat Completions function-calling probe.
    pub chat_function_calling: Option<ToolCallProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Observation from the Responses function-calling probe.
    pub responses_function_calling: Option<ToolCallProbeResult>,
}

#[cfg(test)]
mod tests;
