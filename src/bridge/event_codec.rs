//! Production canonical Event IR bridge for incremental Chat/Responses lowering.
//!
//! This facade decodes one upstream SSE event into canonical events, validates each transition with
//! the pure reducer, and encodes the validated event for the downstream protocol. Transport I/O,
//! retry, routing, downstream commit, and tool execution remain with their existing owners.

use bytes::Bytes;
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ReasoningOutput},
    ir::generation::{
        ChangeAuthorization, ChangeKind, ChangeReason, EventInput, EventLimits, EventState,
        GenerationEvent, GenerationResponse, MaterializeError, ReduceError, SemanticChange,
        SemanticPath, materialize, reduce,
    },
    transport::sse::SseEvent,
};

mod chat;
mod responses;
mod shared;

use chat::{ChatEventDecoder, ChatEventEncoder};
use responses::{ResponsesEventDecoder, ResponsesEventEncoder};

/// Wire Event codec failure before production Bridge takeover.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StaticEventCodecError {
    #[error("wire event is not valid JSON")]
    InvalidJson,
    #[error("wire event uses unsupported semantics")]
    UnsupportedSemantics,
    #[error("wire event lifecycle is invalid")]
    InvalidLifecycle,
    #[error("wire event identity conflicts with prior state")]
    IdentityConflict,
    #[error("wire event repeats a canonical identity")]
    DuplicateIdentity,
    #[error("wire event or encoded target event exceeds its bound")]
    LimitExceeded,
    #[error("wire event stream ended before a protocol terminal")]
    EofBeforeTerminal,
    #[error("canonical Event reducer rejected the transition: {0}")]
    Reduce(#[from] ReduceError),
    #[error("completed Event state cannot materialize: {0}")]
    Materialize(#[from] MaterializeError),
}

enum WireDecoder {
    Chat(ChatEventDecoder),
    Responses(ResponsesEventDecoder),
}

impl WireDecoder {
    fn decode(
        &mut self,
        event: &SseEvent,
    ) -> Result<Vec<crate::ir::generation::EventEnvelope>, StaticEventCodecError> {
        match self {
            Self::Chat(decoder) => decoder.decode(event),
            Self::Responses(decoder) => decoder.decode(event),
        }
    }

    fn finish(&self) -> Result<(), StaticEventCodecError> {
        match self {
            Self::Chat(decoder) => decoder.finish(),
            Self::Responses(decoder) => decoder.finish(),
        }
    }
}

enum WireEncoder {
    Chat(ChatEventEncoder),
    Responses(ResponsesEventEncoder),
}

impl WireEncoder {
    fn encode(&mut self, event: &GenerationEvent) -> Result<Bytes, StaticEventCodecError> {
        match self {
            Self::Chat(encoder) => encoder.encode(event),
            Self::Responses(encoder) => encoder.encode(event),
        }
    }

    fn finish(&self) -> Result<(), StaticEventCodecError> {
        match self {
            Self::Chat(encoder) => encoder.finish(),
            Self::Responses(encoder) => encoder.finish(),
        }
    }
}

/// Per-request production Event IR state for one fixed cross-protocol Bridge.
pub struct StaticEventBridge {
    decoder: WireDecoder,
    encoder: WireEncoder,
    state: Option<EventState>,
    limits: EventLimits,
    encoded_bytes: usize,
    changes: Vec<SemanticChange>,
    finished: bool,
}

impl StaticEventBridge {
    /// Fixes one upstream/downstream protocol direction and bounded canonical turn state.
    pub fn new(
        upstream_protocol: ApiProtocol,
        downstream_protocol: ApiProtocol,
        public_model: &str,
        reasoning_output: ReasoningOutput,
        include_chat_usage: bool,
        limits: EventLimits,
    ) -> Result<Self, StaticEventCodecError> {
        if upstream_protocol == downstream_protocol || public_model.is_empty() {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let decoder = match upstream_protocol {
            ApiProtocol::ChatCompletions => {
                WireDecoder::Chat(ChatEventDecoder::new(limits, reasoning_output))
            }
            ApiProtocol::Responses => WireDecoder::Responses(ResponsesEventDecoder::new(limits)),
        };
        let encoder = match downstream_protocol {
            ApiProtocol::ChatCompletions => WireEncoder::Chat(ChatEventEncoder::new(
                limits,
                public_model,
                include_chat_usage,
                reasoning_output,
            )),
            ApiProtocol::Responses => {
                WireEncoder::Responses(ResponsesEventEncoder::new(limits, public_model))
            }
        };
        Ok(Self {
            decoder,
            encoder,
            state: Some(EventState::new(limits)),
            limits,
            encoded_bytes: 0,
            changes: vec![SemanticChange::new(
                SemanticPath::root(),
                ChangeKind::Normalized,
                ChangeReason::ProtocolNormalized,
                ChangeAuthorization::default(),
            )],
            finished: false,
        })
    }

    /// Decodes, reduces, and encodes one framed upstream SSE event.
    pub fn render(&mut self, event: SseEvent) -> Result<Bytes, StaticEventCodecError> {
        if self.finished {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let events = self.decoder.decode(&event)?;
        let mut output = Vec::new();
        for envelope in events {
            let canonical = envelope.event().clone();
            let opaque_change = if let GenerationEvent::PartDelta {
                part,
                delta: crate::ir::generation::PartDelta::Opaque(_),
            } = &canonical
            {
                Some(SemanticChange::new(
                    SemanticPath::new(format!("parts[{}].opaque", part.id().as_str())),
                    ChangeKind::OpaquePreserved,
                    ChangeReason::OpaqueStatePreserved,
                    ChangeAuthorization::default(),
                ))
            } else {
                None
            };
            let state = self
                .state
                .take()
                .ok_or(StaticEventCodecError::InvalidLifecycle)?;
            self.state = Some(reduce(state, EventInput::Event(Box::new(envelope)))?);
            let encoded = self.encoder.encode(&canonical)?;
            let next = self
                .encoded_bytes
                .checked_add(encoded.len())
                .ok_or(StaticEventCodecError::LimitExceeded)?;
            if next > self.limits.max_turn_bytes() {
                return Err(StaticEventCodecError::LimitExceeded);
            }
            self.encoded_bytes = next;
            if let Some(change) = opaque_change {
                self.changes.push(change);
            }
            output.extend_from_slice(&encoded);
        }
        Ok(Bytes::from(output))
    }

    /// Applies canonical EOF and verifies terminal materialization without emitting extra frames.
    pub fn finish(&mut self) -> Result<Bytes, StaticEventCodecError> {
        if self.finished {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        self.decoder.finish()?;
        let state = self
            .state
            .take()
            .ok_or(StaticEventCodecError::InvalidLifecycle)?;
        let state = reduce(state, EventInput::Eof)?;
        materialize(&state)?;
        self.encoder.finish()?;
        self.state = Some(state);
        self.finished = true;
        Ok(Bytes::new())
    }

    /// Returns the completed canonical response after EOF validation.
    pub fn materialized_response(&self) -> Result<GenerationResponse, StaticEventCodecError> {
        if !self.finished {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        materialize(
            self.state
                .as_ref()
                .ok_or(StaticEventCodecError::InvalidLifecycle)?,
        )
        .map_err(StaticEventCodecError::from)
    }

    /// Returns semantic changes observed during cross-protocol event lowering.
    pub fn changes(&self) -> &[SemanticChange] {
        &self.changes
    }
}
