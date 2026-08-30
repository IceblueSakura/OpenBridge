//! Pure lifecycle reducer for canonical Generation events.

use serde_json::Value;
use thiserror::Error;

use super::algebra::*;
use crate::ir::generation::{JsonObject, MessageRole, Usage};

/// Canonical event lifecycle, identity, bound, or terminal failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ReduceError {
    #[error("event sequence is not the next expected value")]
    InvalidSequence,
    #[error("event sequence overflowed")]
    SequenceOverflow,
    #[error("event arrived after transport EOF")]
    InputAfterEof,
    #[error("event arrived after the canonical terminal")]
    InputAfterTerminal,
    #[error("transport EOF arrived before a canonical terminal")]
    EofBeforeTerminal,
    #[error("transport EOF was supplied more than once")]
    DuplicateEof,
    #[error("event is invalid in the current lifecycle state")]
    InvalidLifecycle,
    #[error("event repeats a canonical identity or output index")]
    DuplicateIdentity,
    #[error("event reference conflicts with its established parent")]
    IdentityConflict,
    #[error("event references an unknown or already-finished value")]
    UnknownReference,
    #[error("delta kind does not match the started part")]
    DeltaKindMismatch,
    #[error("one delta exceeds the configured event limit")]
    EventLimitExceeded,
    #[error("one accumulated part exceeds the configured limit")]
    PartLimitExceeded,
    #[error("the accumulated turn exceeds the configured limit")]
    TurnLimitExceeded,
    #[error("function tool arguments are not one complete JSON object")]
    InvalidToolArguments,
    #[error("a parent finished while one child remained open")]
    IncompleteChildren,
    #[error("the completed item shape does not match its immutable header")]
    InvalidItemShape,
    #[error("usage counters regressed or became missing")]
    UsageRegressed,
}

/// Applies one event or EOF to owned state without hidden I/O or shared mutation.
pub fn reduce(mut state: EventState, input: EventInput) -> Result<EventState, ReduceError> {
    if state.eof == EofState::Clean {
        return match input {
            EventInput::Eof => Err(ReduceError::DuplicateEof),
            EventInput::Event(_) => Err(ReduceError::InputAfterEof),
        };
    }

    match input {
        EventInput::Eof => {
            if state.terminal.is_none() {
                return Err(ReduceError::EofBeforeTerminal);
            }
            state.eof = EofState::Clean;
            Ok(state)
        }
        EventInput::Event(envelope) => {
            if state.terminal.is_some() {
                return Err(ReduceError::InputAfterTerminal);
            }
            let (sequence, event) = envelope.into_parts();
            if sequence.get() != state.next_sequence {
                return Err(ReduceError::InvalidSequence);
            }
            state.next_sequence = state
                .next_sequence
                .checked_add(1)
                .ok_or(ReduceError::SequenceOverflow)?;
            apply_event(&mut state, event)?;
            Ok(state)
        }
    }
}

fn apply_event(state: &mut EventState, event: GenerationEvent) -> Result<(), ReduceError> {
    match event {
        GenerationEvent::ResponseStarted { response } => start_response(state, response),
        GenerationEvent::CandidateStarted { candidate } => start_candidate(state, candidate),
        GenerationEvent::ItemStarted {
            candidate,
            item,
            header,
        } => start_item(state, candidate, item, header),
        GenerationEvent::PartStarted { item, part, kind } => start_part(state, item, part, kind),
        GenerationEvent::PartDelta { part, delta } => append_delta(state, part, delta),
        GenerationEvent::PartFinished { part } => finish_part(state, part),
        GenerationEvent::ItemFinished { item } => finish_item(state, item),
        GenerationEvent::CandidateFinished { candidate, finish } => {
            finish_candidate(state, candidate, finish)
        }
        GenerationEvent::UsageSnapshot { usage } => update_usage(state, usage),
        GenerationEvent::Terminal { terminal } => finish_turn(state, terminal),
    }
}

fn start_response(state: &mut EventState, response: ResponseIdentity) -> Result<(), ReduceError> {
    if state.response.is_some()
        || !state.candidates.is_empty()
        || !state.items.is_empty()
        || !state.parts.is_empty()
    {
        return Err(ReduceError::InvalidLifecycle);
    }
    state.response = Some(response);
    Ok(())
}

fn start_candidate(
    state: &mut EventState,
    candidate: CandidateIdentity,
) -> Result<(), ReduceError> {
    if state.response.is_none() {
        return Err(ReduceError::InvalidLifecycle);
    }
    if state.candidates.contains_key(candidate.id())
        || state.candidate_indexes.contains_key(&candidate.index())
    {
        return Err(ReduceError::DuplicateIdentity);
    }
    state
        .candidate_indexes
        .insert(candidate.index(), candidate.id().clone());
    state.candidates.insert(
        candidate.id().clone(),
        CandidateBuilder {
            identity: candidate,
            items: Default::default(),
            finished: false,
            finish: None,
        },
    );
    Ok(())
}

fn start_item(
    state: &mut EventState,
    candidate: CandidateRef,
    item: ItemIdentity,
    header: ItemHeader,
) -> Result<(), ReduceError> {
    let candidate_builder = state
        .candidates
        .get_mut(candidate.id())
        .ok_or(ReduceError::UnknownReference)?;
    if candidate_builder.finished {
        return Err(ReduceError::UnknownReference);
    }
    if state.items.contains_key(item.id()) || candidate_builder.items.contains_key(&item.index()) {
        return Err(ReduceError::DuplicateIdentity);
    }
    candidate_builder
        .items
        .insert(item.index(), item.id().clone());
    state.items.insert(
        item.id().clone(),
        ItemBuilder {
            candidate: candidate.id().clone(),
            identity: item,
            header,
            parts: Default::default(),
            finished: false,
        },
    );
    Ok(())
}

fn start_part(
    state: &mut EventState,
    item: ItemRef,
    part: PartIdentity,
    kind: PartKind,
) -> Result<(), ReduceError> {
    let item_builder = state
        .items
        .get_mut(item.id())
        .ok_or(ReduceError::UnknownReference)?;
    if item_builder.finished {
        return Err(ReduceError::UnknownReference);
    }
    if !header_accepts_part(&item_builder.header, kind) {
        return Err(ReduceError::InvalidItemShape);
    }
    if state.parts.contains_key(part.id()) || item_builder.parts.contains_key(&part.index()) {
        return Err(ReduceError::DuplicateIdentity);
    }
    item_builder.parts.insert(part.index(), part.id().clone());
    state.parts.insert(
        part.id().clone(),
        PartBuilder {
            item: item.id().clone(),
            identity: part,
            kind,
            value: String::new(),
            opaque: None,
            parsed_arguments: None,
            finished: false,
        },
    );
    Ok(())
}

fn header_accepts_part(header: &ItemHeader, kind: PartKind) -> bool {
    matches!(
        (header, kind),
        (ItemHeader::Message { .. }, PartKind::Text)
            | (
                ItemHeader::Reasoning,
                PartKind::ReasoningText | PartKind::ReasoningSummary | PartKind::Opaque
            )
            | (ItemHeader::ToolCall { .. }, PartKind::ToolArguments)
    )
}

fn append_delta(
    state: &mut EventState,
    part: PartRef,
    delta: PartDelta,
) -> Result<(), ReduceError> {
    let delta_len = delta.encoded_len();
    if delta_len > state.limits.max_event_bytes() {
        return Err(ReduceError::EventLimitExceeded);
    }
    let builder = state
        .parts
        .get_mut(part.id())
        .ok_or(ReduceError::UnknownReference)?;
    if builder.finished {
        return Err(ReduceError::UnknownReference);
    }
    if !delta.accepts(builder.kind) {
        return Err(ReduceError::DeltaKindMismatch);
    }
    let current_len = builder
        .value
        .len()
        .checked_add(builder.opaque.as_ref().map_or(0, |value| value.len()))
        .ok_or(ReduceError::PartLimitExceeded)?;
    let part_len = current_len
        .checked_add(delta_len)
        .ok_or(ReduceError::PartLimitExceeded)?;
    if part_len > state.limits.max_part_bytes() {
        return Err(ReduceError::PartLimitExceeded);
    }
    let turn_len = state
        .turn_bytes
        .checked_add(delta_len)
        .ok_or(ReduceError::TurnLimitExceeded)?;
    if turn_len > state.limits.max_turn_bytes() {
        return Err(ReduceError::TurnLimitExceeded);
    }
    match (delta.text(), delta.opaque()) {
        (Some(value), None) => builder.value.push_str(value),
        (None, Some(value)) if builder.opaque.is_none() && builder.value.is_empty() => {
            builder.opaque = Some(value.clone());
        }
        _ => return Err(ReduceError::DeltaKindMismatch),
    }
    state.turn_bytes = turn_len;
    Ok(())
}

fn finish_part(state: &mut EventState, part: PartRef) -> Result<(), ReduceError> {
    let builder = state
        .parts
        .get_mut(part.id())
        .ok_or(ReduceError::UnknownReference)?;
    if builder.finished {
        return Err(ReduceError::UnknownReference);
    }
    if builder.value.is_empty() && builder.opaque.is_none() {
        return Err(ReduceError::InvalidItemShape);
    }
    if builder.kind == PartKind::ToolArguments {
        let value: Value =
            serde_json::from_str(&builder.value).map_err(|_| ReduceError::InvalidToolArguments)?;
        builder.parsed_arguments = Some(
            JsonObject::new(value, state.limits.max_part_bytes())
                .map_err(|_| ReduceError::InvalidToolArguments)?,
        );
    }
    builder.finished = true;
    Ok(())
}

fn finish_item(state: &mut EventState, item: ItemRef) -> Result<(), ReduceError> {
    let builder = state
        .items
        .get(item.id())
        .ok_or(ReduceError::UnknownReference)?;
    if builder.finished {
        return Err(ReduceError::UnknownReference);
    }
    if builder.parts.is_empty() {
        return Err(ReduceError::InvalidItemShape);
    }
    let parts = builder
        .parts
        .values()
        .map(|id| state.parts.get(id).ok_or(ReduceError::IdentityConflict))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.iter().any(|part| !part.finished) {
        return Err(ReduceError::IncompleteChildren);
    }
    match &builder.header {
        ItemHeader::Message { role } => {
            if *role != MessageRole::Assistant
                || parts.iter().any(|part| part.kind != PartKind::Text)
            {
                return Err(ReduceError::InvalidItemShape);
            }
        }
        ItemHeader::Reasoning => {
            if parts.iter().any(|part| {
                !matches!(
                    part.kind,
                    PartKind::ReasoningText | PartKind::ReasoningSummary | PartKind::Opaque
                )
            }) {
                return Err(ReduceError::InvalidItemShape);
            }
        }
        ItemHeader::ToolCall { .. } => {
            if parts.len() != 1
                || parts[0].kind != PartKind::ToolArguments
                || parts[0].parsed_arguments.is_none()
            {
                return Err(ReduceError::InvalidItemShape);
            }
        }
    }
    state.items.get_mut(item.id()).unwrap().finished = true;
    Ok(())
}

fn finish_candidate(
    state: &mut EventState,
    candidate: CandidateRef,
    finish: crate::ir::generation::FinishReason,
) -> Result<(), ReduceError> {
    let builder = state
        .candidates
        .get(candidate.id())
        .ok_or(ReduceError::UnknownReference)?;
    if builder.finished {
        return Err(ReduceError::UnknownReference);
    }
    if builder.items.is_empty() && finish != crate::ir::generation::FinishReason::Stop {
        return Err(ReduceError::InvalidItemShape);
    }
    if builder
        .items
        .values()
        .any(|id| state.items.get(id).is_none_or(|item| !item.finished))
    {
        return Err(ReduceError::IncompleteChildren);
    }
    let builder = state.candidates.get_mut(candidate.id()).unwrap();
    builder.finished = true;
    builder.finish = Some(finish);
    Ok(())
}

fn update_usage(state: &mut EventState, usage: Usage) -> Result<(), ReduceError> {
    if state.response.is_none() {
        return Err(ReduceError::InvalidLifecycle);
    }
    if state
        .usage
        .as_ref()
        .is_some_and(|previous| !usage_progresses(previous, &usage))
    {
        return Err(ReduceError::UsageRegressed);
    }
    state.usage = Some(usage);
    Ok(())
}

fn usage_progresses(previous: &Usage, next: &Usage) -> bool {
    counter_progresses(previous.input_tokens(), next.input_tokens())
        && counter_progresses(previous.output_tokens(), next.output_tokens())
        && counter_progresses(previous.total_tokens(), next.total_tokens())
        && counter_progresses(previous.reasoning_tokens(), next.reasoning_tokens())
        && counter_progresses(previous.cached_input_tokens(), next.cached_input_tokens())
}

fn counter_progresses(previous: Option<u64>, next: Option<u64>) -> bool {
    match previous {
        None => true,
        Some(previous) => next.is_some_and(|next| next >= previous),
    }
}

fn finish_turn(state: &mut EventState, terminal: TurnTerminal) -> Result<(), ReduceError> {
    if state.response.is_none() {
        return Err(ReduceError::InvalidLifecycle);
    }
    if terminal.status() == TerminalStatus::Completed
        && (state.candidates.is_empty()
            || state
                .candidates
                .values()
                .any(|candidate| !candidate.finished)
            || state.items.values().any(|item| !item.finished)
            || state.parts.values().any(|part| !part.finished))
    {
        return Err(ReduceError::IncompleteChildren);
    }
    state.terminal = Some(terminal);
    Ok(())
}
