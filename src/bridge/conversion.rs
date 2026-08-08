//! Public plan and renderer facade for restricted bidirectional Chat Completions and Responses conversion.
//!
//! Request, non-streaming response, streaming response, and shared wire helpers live in private
//! submodules. This module owns the stable boundary for `BridgePlan`, `BridgeStreamRenderer`,
//! and errors; it never executes tools or relaxes fail-closed rules for Provider extensions or
//! unmodeled semantics.

use bytes::Bytes;
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ApiRequest, ReasoningOutput},
    transport::sse::SseEvent,
};

use super::BridgeStreamError;
use request::{chat_request_to_responses, reject_unsupported_request, responses_request_to_chat};
use response::{chat_response_to_responses, responses_response_to_chat};
use shared::parse_value_object;
use stream::{ChatToResponsesStream, ResponsesToChatStream};

mod request;
mod response;
mod shared;
mod stream;

/// Error returned when a request, response, or stream cannot be converted under the restricted Bridge contract.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BridgeError {
    /// The input is not a JSON object valid for the requested direction.
    #[error("bridge input is not a valid protocol object")]
    InvalidShape,
    /// The input uses semantics not declared as Bridge-supported.
    #[error("bridge input uses unsupported semantics")]
    UnsupportedSemantics,
    /// A function call/result identity is missing, duplicated, or unmatchable.
    #[error("bridge tool identity is invalid")]
    InvalidToolIdentity,
    /// Function arguments are not a closed JSON object.
    #[error("bridge function arguments are invalid")]
    InvalidToolArguments,
    /// The upstream stream lifecycle failed.
    #[error("bridge stream lifecycle is invalid")]
    InvalidStream,
}

impl From<BridgeStreamError> for BridgeError {
    fn from(_: BridgeStreamError) -> Self {
        Self::InvalidStream
    }
}

/// Execution plan with a fixed conversion direction, Public Model, and upstream model.
#[derive(Clone, Debug)]
pub struct BridgePlan {
    downstream_protocol: ApiProtocol,
    upstream_protocol: ApiProtocol,
    public_model: String,
    reasoning_supported: bool,
}

impl BridgePlan {
    /// Validates and converts a downstream request into an immutable plan and upstream protocol request.
    pub fn prepare(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        Self::prepare_with_reasoning_output(
            downstream_protocol,
            upstream_protocol,
            public_model,
            upstream_model,
            body,
            ReasoningOutput::Unsupported,
        )
    }

    /// Validates and converts a request, allowing reasoning to cross protocols only when the upstream
    /// protocol declares a representable output type.
    pub fn prepare_with_reasoning_output(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        reasoning_output: ReasoningOutput,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        // Pass only reasoning that the current upstream protocol can safely represent to the directional converter.
        let reasoning_supported = bridge_reasoning_supported(upstream_protocol, reasoning_output);

        // Reject same-protocol calls and unsupported extensions before running directional conversion.
        if downstream_protocol == upstream_protocol {
            return Err(BridgeError::UnsupportedSemantics);
        }
        let source = parse_value_object(&body)?;
        reject_unsupported_request(downstream_protocol, &source)?;
        let converted = match (downstream_protocol, upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => chat_request_to_responses(
                &source,
                upstream_model,
                reasoning_supported,
                reasoning_output == ReasoningOutput::Summary,
            )?,
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                responses_request_to_chat(&source, upstream_model, reasoning_supported)?
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        };

        // Fix the downstream facts required for response conversion and pass compact JSON to the Provider adapter.
        let request = ApiRequest::new(
            upstream_protocol,
            Bytes::from(serde_json::to_vec(&converted).map_err(|_| BridgeError::InvalidShape)?),
        );
        Ok((
            Self {
                downstream_protocol,
                upstream_protocol,
                public_model: public_model.to_owned(),
                reasoning_supported,
            },
            request,
        ))
    }

    /// Returns the plan's downstream protocol.
    pub fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_protocol
    }

    /// Returns the protocol actually called by the plan.
    pub fn upstream_protocol(&self) -> ApiProtocol {
        self.upstream_protocol
    }

    /// Converts a complete successful upstream JSON response to the downstream protocol.
    pub fn render_non_stream(&self, body: Bytes) -> Result<Bytes, BridgeError> {
        // Parse the upstream object and build the downstream response for the fixed direction.
        let source = parse_value_object(&body)?;
        let converted = match (self.downstream_protocol, self.upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                responses_response_to_chat(&source, &self.public_model, self.reasoning_supported)?
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                chat_response_to_responses(&source, &self.public_model, self.reasoning_supported)?
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        };
        serde_json::to_vec(&converted)
            .map(Bytes::from)
            .map_err(|_| BridgeError::InvalidShape)
    }

    /// Creates an incremental SSE renderer dedicated to this request.
    pub fn stream_renderer(&self) -> BridgeStreamRenderer {
        BridgeStreamRenderer::new(self.clone())
    }
}

/// Determines whether upstream reasoning output is safely representable in the current Bridge direction.
fn bridge_reasoning_supported(
    upstream_protocol: ApiProtocol,
    reasoning_output: ReasoningOutput,
) -> bool {
    match upstream_protocol {
        ApiProtocol::ChatCompletions => reasoning_output == ReasoningOutput::PlainText,
        ApiProtocol::Responses => reasoning_output.is_readable(),
    }
}

/// Renders a complete upstream SSE event into downstream protocol events.
pub struct BridgeStreamRenderer {
    plan: BridgePlan,
    state: StreamState,
}

enum StreamState {
    ResponsesToChat(ResponsesToChatStream),
    ChatToResponses(ChatToResponsesStream),
}

impl BridgeStreamRenderer {
    fn new(plan: BridgePlan) -> Self {
        let state = match (plan.downstream_protocol, plan.upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                StreamState::ResponsesToChat(ResponsesToChatStream::new(plan.reasoning_supported))
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                StreamState::ChatToResponses(ChatToResponsesStream::new(plan.reasoning_supported))
            }
            _ => unreachable!("BridgePlan always has opposite protocols"),
        };
        Self { plan, state }
    }

    /// Consumes one fully framed upstream event and returns zero or more downstream SSE event bytes.
    pub fn render(&mut self, event: SseEvent) -> Result<Bytes, BridgeError> {
        match &mut self.state {
            StreamState::ResponsesToChat(state) => state.render(event, &self.plan.public_model),
            StreamState::ChatToResponses(state) => state.render(event, &self.plan.public_model),
        }
    }

    /// Ends upstream input and confirms that an explicit terminal has arrived.
    pub fn finish(&mut self) -> Result<Bytes, BridgeError> {
        match &mut self.state {
            StreamState::ResponsesToChat(state) => state.finish(),
            StreamState::ChatToResponses(state) => state.finish(),
        }
    }
}
