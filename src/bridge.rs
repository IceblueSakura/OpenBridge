//! Protocol Bridge facade backed exclusively by canonical Generation Static/Event IR.
//!
//! The Bridge owns pure Chat ↔ Responses lowering and per-request Event state. Routing, Provider
//! selection, transport I/O, precommit, liveness, cancellation, and downstream commit remain with
//! their existing owners.

mod event_codec;
mod responses;
mod shared;
mod static_codec;

pub use event_codec::{StaticEventBridge, StaticEventCodecError};
pub use responses::ResponsesStreamState;
pub use static_codec::{
    BridgeError, BridgeLimits, BridgePlan, BridgeStreamRenderer, ProviderToolTarget,
    StaticBridgePlan, StaticCodecError, StaticCodecLimits, StaticRenderedResponse,
};

/// Native Responses buffering terminal retained until the R7 Native takeover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    Completed,
    Failed,
    Incomplete,
    Cancelled,
    Error,
}

/// Native Responses buffering lifecycle failure retained until the R7 Native takeover.
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum BridgeStreamError {
    #[error("buffered Responses event data is invalid JSON")]
    InvalidJson,
    #[error("SSE event name conflicts with the JSON event type")]
    EventTypeConflict,
    #[error("buffered Responses event is invalid in the current lifecycle state")]
    UnexpectedEvent,
    #[error("buffered Responses event repeats an existing identity")]
    DuplicateIdentity,
    #[error("buffered Responses event conflicts with an established identity")]
    IdentityConflict,
    #[error("buffered Responses event references an unknown output item")]
    UnknownOutputItem,
    #[error("buffered Responses function arguments are incomplete or invalid")]
    InvalidToolArguments,
    #[error("buffered Responses terminal arrived before output completion")]
    IncompleteOutputItem,
    #[error("buffered Responses stream contains more than one terminal")]
    DuplicateTerminal,
    #[error("buffered Responses stream ended before an explicit terminal")]
    EofBeforeTerminal,
}

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
    pub fn output_index(&self) -> u64 {
        self.output_index
    }

    pub fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

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
