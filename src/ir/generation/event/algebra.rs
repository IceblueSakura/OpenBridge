//! Canonical Event IR values for one Provider generation turn.

use std::collections::BTreeMap;

use thiserror::Error;

use super::super::{
    BoundedBytes, CallId, CandidateId, FinishReason, IdentityValidationError, ItemId, JsonObject,
    MessageRole, ResponseId, TextValue, ToolName, Usage, WireIdentity,
};

/// Validation failure for fixed Event IR limits.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EventValidationError {
    /// Every event admission limit must be non-zero.
    #[error("event limits must be non-zero")]
    ZeroLimit,
}

/// Resource limits enforced by the canonical reducer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventLimits {
    max_event_bytes: usize,
    max_part_bytes: usize,
    max_turn_bytes: usize,
}

impl EventLimits {
    /// Creates non-zero event, part, and whole-turn bounds.
    pub fn new(
        max_event_bytes: usize,
        max_part_bytes: usize,
        max_turn_bytes: usize,
    ) -> Result<Self, EventValidationError> {
        if max_event_bytes == 0 || max_part_bytes == 0 || max_turn_bytes == 0 {
            return Err(EventValidationError::ZeroLimit);
        }
        Ok(Self {
            max_event_bytes,
            max_part_bytes,
            max_turn_bytes,
        })
    }

    /// Returns the maximum bytes accepted for one decoded or encoded event payload.
    pub const fn max_event_bytes(self) -> usize {
        self.max_event_bytes
    }

    /// Returns the maximum accumulated bytes for one content part.
    pub const fn max_part_bytes(self) -> usize {
        self.max_part_bytes
    }

    /// Returns the maximum accumulated content bytes for one turn.
    pub const fn max_turn_bytes(self) -> usize {
        self.max_turn_bytes
    }
}

/// Strictly monotonic sequence assigned by one wire decoder.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Sequence(u64);

impl Sequence {
    /// Creates a sequence value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the sequence number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Protocol ordering index kept separate from canonical identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OutputIndex(u64);

impl OutputIndex {
    /// Creates an output index.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the index value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Canonical identity for one streamed content part.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PartId(String);

impl PartId {
    /// Creates a non-empty bounded part identity.
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, IdentityValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdentityValidationError::Empty);
        }
        if value.len() > max_bytes {
            return Err(IdentityValidationError::TooLarge { max_bytes });
        }
        Ok(Self(value))
    }

    /// Returns the canonical part identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Response lifecycle identity fixed by `ResponseStarted`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseIdentity {
    id: ResponseId,
}

impl ResponseIdentity {
    /// Creates a response identity.
    pub const fn new(id: ResponseId) -> Self {
        Self { id }
    }

    /// Returns the canonical response identity.
    pub const fn id(&self) -> &ResponseId {
        &self.id
    }
}

/// Candidate lifecycle identity fixed by `CandidateStarted`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateIdentity {
    id: CandidateId,
    index: OutputIndex,
}

impl CandidateIdentity {
    /// Creates a candidate identity and independent output index.
    pub const fn new(id: CandidateId, index: OutputIndex) -> Self {
        Self { id, index }
    }

    /// Returns the canonical candidate identity.
    pub const fn id(&self) -> &CandidateId {
        &self.id
    }

    /// Returns the source ordering index.
    pub const fn index(&self) -> OutputIndex {
        self.index
    }
}

/// Reference to an already-started candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateRef {
    id: CandidateId,
}

impl CandidateRef {
    /// Creates a candidate reference.
    pub const fn new(id: CandidateId) -> Self {
        Self { id }
    }

    /// Returns the referenced candidate identity.
    pub const fn id(&self) -> &CandidateId {
        &self.id
    }
}

/// Item lifecycle identity fixed by `ItemStarted`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemIdentity {
    id: ItemId,
    index: OutputIndex,
    wire_identity: Option<WireIdentity>,
}

impl ItemIdentity {
    /// Creates an item identity and optional Provider wire identity.
    pub fn new(id: ItemId, index: OutputIndex, wire_identity: Option<WireIdentity>) -> Self {
        Self {
            id,
            index,
            wire_identity,
        }
    }

    /// Returns the canonical item identity.
    pub const fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns the source ordering index.
    pub const fn index(&self) -> OutputIndex {
        self.index
    }

    /// Returns the optional Provider wire identity.
    pub const fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}

/// Reference to an already-started item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemRef {
    id: ItemId,
}

impl ItemRef {
    /// Creates an item reference.
    pub const fn new(id: ItemId) -> Self {
        Self { id }
    }

    /// Returns the referenced item identity.
    pub const fn id(&self) -> &ItemId {
        &self.id
    }
}

/// Part lifecycle identity fixed by `PartStarted`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartIdentity {
    id: PartId,
    index: OutputIndex,
}

impl PartIdentity {
    /// Creates a part identity and independent ordering index.
    pub const fn new(id: PartId, index: OutputIndex) -> Self {
        Self { id, index }
    }

    /// Returns the canonical part identity.
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// Returns the source ordering index.
    pub const fn index(&self) -> OutputIndex {
        self.index
    }
}

/// Reference to an already-started part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartRef {
    id: PartId,
}

impl PartRef {
    /// Creates a part reference.
    pub const fn new(id: PartId) -> Self {
        Self { id }
    }

    /// Returns the referenced part identity.
    pub const fn id(&self) -> &PartId {
        &self.id
    }
}

/// Immutable semantics fixed when one output item begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemHeader {
    /// Assistant message output.
    Message { role: MessageRole },
    /// Reasoning output kept separate from assistant text.
    Reasoning,
    /// Completed function-call metadata; arguments arrive through a part.
    ToolCall { call: CallId, tool: ToolName },
}

/// Closed part category fixed at part start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartKind {
    /// Visible assistant text.
    Text,
    /// Readable reasoning text.
    ReasoningText,
    /// Readable reasoning summary.
    ReasoningSummary,
    /// Incremental JSON function arguments.
    ToolArguments,
    /// Provider-owned opaque reasoning state.
    Opaque,
}

/// One bounded incremental part payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PartDelta {
    /// Visible assistant text.
    Text(TextValue),
    /// Readable reasoning text.
    ReasoningText(TextValue),
    /// Readable reasoning summary.
    ReasoningSummary(TextValue),
    /// Raw JSON fragment whose complete value is parsed at part finish.
    ToolArguments(TextValue),
    /// Complete opaque payload retained without interpretation.
    Opaque(BoundedBytes),
}

impl PartDelta {
    pub(super) fn encoded_len(&self) -> usize {
        match self {
            Self::Text(value)
            | Self::ReasoningText(value)
            | Self::ReasoningSummary(value)
            | Self::ToolArguments(value) => value.as_str().len(),
            Self::Opaque(value) => value.len(),
        }
    }

    pub(super) fn accepts(&self, kind: PartKind) -> bool {
        matches!(
            (self, kind),
            (Self::Text(_), PartKind::Text)
                | (Self::ReasoningText(_), PartKind::ReasoningText)
                | (Self::ReasoningSummary(_), PartKind::ReasoningSummary)
                | (Self::ToolArguments(_), PartKind::ToolArguments)
                | (Self::Opaque(_), PartKind::Opaque)
        )
    }

    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(value)
            | Self::ReasoningText(value)
            | Self::ReasoningSummary(value)
            | Self::ToolArguments(value) => Some(value.as_str()),
            Self::Opaque(_) => None,
        }
    }

    pub(super) fn opaque(&self) -> Option<&BoundedBytes> {
        match self {
            Self::Opaque(value) => Some(value),
            _ => None,
        }
    }
}

/// Provider-declared terminal status for one turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStatus {
    /// Turn completed successfully.
    Completed,
    /// Provider reported failure.
    Failed,
    /// Provider stopped before completing the request.
    Incomplete,
    /// Provider cancelled the turn.
    Cancelled,
    /// Provider reported a protocol-level error terminal.
    Error,
}

/// Terminal status and optional bounded failure detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurnTerminal {
    status: TerminalStatus,
    failure: Option<TextValue>,
}

impl TurnTerminal {
    /// Creates a terminal value.
    pub fn new(status: TerminalStatus, failure: Option<TextValue>) -> Self {
        Self { status, failure }
    }

    /// Returns the terminal status.
    pub const fn status(&self) -> TerminalStatus {
        self.status
    }

    /// Returns the optional failure detail.
    pub const fn failure(&self) -> Option<&TextValue> {
        self.failure.as_ref()
    }
}

/// One canonical generation lifecycle event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationEvent {
    ResponseStarted {
        response: ResponseIdentity,
    },
    CandidateStarted {
        candidate: CandidateIdentity,
    },
    ItemStarted {
        candidate: CandidateRef,
        item: ItemIdentity,
        header: ItemHeader,
    },
    PartStarted {
        item: ItemRef,
        part: PartIdentity,
        kind: PartKind,
    },
    PartDelta {
        part: PartRef,
        delta: PartDelta,
    },
    PartFinished {
        part: PartRef,
    },
    ItemFinished {
        item: ItemRef,
    },
    CandidateFinished {
        candidate: CandidateRef,
        finish: FinishReason,
    },
    UsageSnapshot {
        usage: Usage,
    },
    Terminal {
        terminal: TurnTerminal,
    },
}

/// Sequenced event produced by a wire decoder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEnvelope {
    sequence: Sequence,
    event: GenerationEvent,
}

impl EventEnvelope {
    /// Creates a sequenced event.
    pub const fn new(sequence: Sequence, event: GenerationEvent) -> Self {
        Self { sequence, event }
    }

    /// Returns the event sequence.
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    /// Returns the canonical event.
    pub const fn event(&self) -> &GenerationEvent {
        &self.event
    }

    pub(super) fn into_parts(self) -> (Sequence, GenerationEvent) {
        (self.sequence, self.event)
    }
}

/// Reducer input: one canonical event or transport EOF.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventInput {
    /// One decoded canonical event.
    Event(Box<EventEnvelope>),
    /// End of the framed upstream body.
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EofState {
    Open,
    Clean,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CandidateBuilder {
    pub(super) identity: CandidateIdentity,
    pub(super) items: BTreeMap<OutputIndex, ItemId>,
    pub(super) finished: bool,
    pub(super) finish: Option<FinishReason>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ItemBuilder {
    pub(super) candidate: CandidateId,
    pub(super) identity: ItemIdentity,
    pub(super) header: ItemHeader,
    pub(super) parts: BTreeMap<OutputIndex, PartId>,
    pub(super) finished: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PartBuilder {
    pub(super) item: ItemId,
    pub(super) identity: PartIdentity,
    pub(super) kind: PartKind,
    pub(super) value: String,
    pub(super) opaque: Option<BoundedBytes>,
    pub(super) parsed_arguments: Option<JsonObject>,
    pub(super) finished: bool,
}

/// Fully owned reducer state for one Provider turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventState {
    pub(super) limits: EventLimits,
    pub(super) response: Option<ResponseIdentity>,
    pub(super) candidates: BTreeMap<CandidateId, CandidateBuilder>,
    pub(super) candidate_indexes: BTreeMap<OutputIndex, CandidateId>,
    pub(super) items: BTreeMap<ItemId, ItemBuilder>,
    pub(super) parts: BTreeMap<PartId, PartBuilder>,
    pub(super) next_sequence: u64,
    pub(super) turn_bytes: usize,
    pub(super) usage: Option<Usage>,
    pub(super) terminal: Option<TurnTerminal>,
    pub(super) eof: EofState,
}

impl EventState {
    /// Creates an empty state for one turn.
    pub fn new(limits: EventLimits) -> Self {
        Self {
            limits,
            response: None,
            candidates: BTreeMap::new(),
            candidate_indexes: BTreeMap::new(),
            items: BTreeMap::new(),
            parts: BTreeMap::new(),
            next_sequence: 0,
            turn_bytes: 0,
            usage: None,
            terminal: None,
            eof: EofState::Open,
        }
    }

    /// Returns the current terminal, if one was accepted.
    pub const fn terminal(&self) -> Option<&TurnTerminal> {
        self.terminal.as_ref()
    }

    /// Returns the latest normalized usage snapshot.
    pub const fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }
}
