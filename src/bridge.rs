//! Facade for the explicit bridge state machines that reconcile Chat Completions and Responses streams.
//!
//! Chat and Responses state machines live in private modules and share terminal, tool-identity,
//! and error contracts. The `conversion` module builds `BridgePlan` values and bidirectional
//! renderers. This module does not execute tools, persist a continuation ledger, or translate
//! Provider-private semantics outside the explicit allowlist.

use thiserror::Error;

mod chat;
mod conversion;
mod responses;
mod shared;
mod static_codec;

pub(crate) use chat::ChatStreamEventKind;
pub use chat::ChatStreamState;
pub use conversion::{BridgeError, BridgePlan, BridgeStreamRenderer};
pub use responses::ResponsesStreamState;
pub use static_codec::{
    StaticBridgePlan, StaticCodecError, StaticCodecLimits, StaticRenderedResponse,
};

/// The only terminal state for a bridge stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    /// The upstream protocol explicitly reports successful completion.
    Completed,
    /// Responses explicitly reports failure.
    Failed,
    /// Responses explicitly reports incomplete completion.
    Incomplete,
    /// Responses explicitly reports cancellation.
    Cancelled,
    /// Responses reports failure through a standalone `error` event.
    Error,
}

/// Bridge stream lifecycle or identity validation failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BridgeStreamError {
    /// SSE data is not valid JSON.
    #[error("bridge event data is not valid JSON")]
    InvalidJson,
    /// The SSE `event` name conflicts with the JSON `type`.
    #[error("SSE event name conflicts with the JSON event type")]
    EventTypeConflict,
    /// The event is not accepted in the current lifecycle stage.
    #[error("bridge event is not valid in the current lifecycle state")]
    UnexpectedEvent,
    /// An output index, item ID, or call ID was registered more than once.
    #[error("bridge event repeats an existing identity")]
    DuplicateIdentity,
    /// A later fragment attempts to replace an established identity.
    #[error("bridge event conflicts with an established identity")]
    IdentityConflict,
    /// A delta references an output item that has not been registered.
    #[error("bridge event references an unknown output item")]
    UnknownOutputItem,
    /// Function arguments are incomplete or are not a closed JSON object.
    #[error("function tool arguments are incomplete or not a JSON object")]
    InvalidToolArguments,
    /// The terminal arrives while an output item is still incomplete.
    #[error("bridge terminal arrived before all output items completed")]
    IncompleteOutputItem,
    /// The stream contains more than one terminal.
    #[error("bridge stream contains more than one terminal")]
    DuplicateTerminal,
    /// Input EOF cannot replace the protocol terminal.
    #[error("bridge stream ended before an explicit terminal")]
    EofBeforeTerminal,
}

/// A function tool call reconstructed by a bridge state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeToolCall {
    output_index: u64,
    item_id: Option<String>,
    call_id: String,
    name: String,
    arguments: String,
    completed: bool,
}

impl BridgeToolCall {
    /// Returns the output index from the protocol.
    pub fn output_index(&self) -> u64 {
        self.output_index
    }

    /// Returns the Responses item ID; native Chat streams do not have this identity.
    pub fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }

    /// Returns the stable call ID used across the tool-result round trip.
    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// Returns the function tool name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns arguments concatenated in wire order and verified as a closed JSON object.
    pub fn arguments(&self) -> &str {
        &self.arguments
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    AwaitingStart,
    Streaming,
    Terminal(StreamTerminal),
}
