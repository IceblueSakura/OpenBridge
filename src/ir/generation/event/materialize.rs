//! Pure materialization from a completed Event IR state into Static Generation IR.

use thiserror::Error;

use super::algebra::*;
use crate::ir::generation::{
    Candidate, ContentPart, GenerationResponse, MessageRole, OpaqueExposure, OpaqueKind,
    OpaqueState, OutputItem, ProviderNamespace, ReasoningItem, ReasoningPart, ResponseMessage,
    ResponseStatus, TextValue, ToolCall, ToolInput,
};

/// Failure to construct one complete Static response from Event IR.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MaterializeError {
    #[error("event state has no terminal")]
    MissingTerminal,
    #[error("only a completed terminal can materialize a success response")]
    NonCompletedTerminal,
    #[error("event state is incomplete or internally inconsistent")]
    InvalidState,
    #[error("event payload cannot enter the validated Static IR")]
    InvalidValue,
}

/// Materializes a successfully completed canonical turn without parsing wire JSON.
pub fn materialize(state: &EventState) -> Result<GenerationResponse, MaterializeError> {
    let terminal = state
        .terminal
        .as_ref()
        .ok_or(MaterializeError::MissingTerminal)?;
    if terminal.status() != TerminalStatus::Completed {
        return Err(MaterializeError::NonCompletedTerminal);
    }
    let response = state
        .response
        .as_ref()
        .ok_or(MaterializeError::InvalidState)?;

    let candidates = state
        .candidate_indexes
        .values()
        .map(|candidate_id| materialize_candidate(state, candidate_id))
        .collect::<Result<Vec<_>, _>>()?;
    if candidates.is_empty() {
        return Err(MaterializeError::InvalidState);
    }

    GenerationResponse::new(
        response.id().clone(),
        candidates,
        ResponseStatus::Completed,
        state.usage,
        Vec::new(),
    )
    .map_err(|_| MaterializeError::InvalidState)
}

fn materialize_candidate(
    state: &EventState,
    candidate_id: &crate::ir::generation::CandidateId,
) -> Result<Candidate, MaterializeError> {
    let candidate = state
        .candidates
        .get(candidate_id)
        .filter(|candidate| candidate.finished)
        .ok_or(MaterializeError::InvalidState)?;
    let output = candidate
        .items
        .values()
        .map(|item_id| materialize_item(state, item_id))
        .collect::<Result<Vec<_>, _>>()?;
    Candidate::new(
        candidate.identity.id().clone(),
        output,
        candidate.finish.clone(),
    )
    .map_err(|_| MaterializeError::InvalidState)
}

fn materialize_item(
    state: &EventState,
    item_id: &crate::ir::generation::ItemId,
) -> Result<OutputItem, MaterializeError> {
    let item = state
        .items
        .get(item_id)
        .filter(|item| item.finished)
        .ok_or(MaterializeError::InvalidState)?;
    let parts = item
        .parts
        .values()
        .map(|part_id| {
            state
                .parts
                .get(part_id)
                .filter(|part| part.finished)
                .ok_or(MaterializeError::InvalidState)
        })
        .collect::<Result<Vec<_>, _>>()?;

    match &item.header {
        ItemHeader::Message {
            role: MessageRole::Assistant,
        } => {
            let content = parts
                .iter()
                .map(|part| {
                    if part.kind != PartKind::Text {
                        return Err(MaterializeError::InvalidState);
                    }
                    bounded_text(part, state).map(ContentPart::text)
                })
                .collect::<Result<Vec<_>, _>>()?;
            ResponseMessage::new(
                item.identity.id().clone(),
                content,
                item.identity.wire_identity().cloned(),
            )
            .map(OutputItem::Message)
            .map_err(|_| MaterializeError::InvalidValue)
        }
        ItemHeader::Message { .. } => Err(MaterializeError::InvalidState),
        ItemHeader::Reasoning => {
            let reasoning = parts
                .iter()
                .map(|part| match part.kind {
                    PartKind::ReasoningText => {
                        bounded_text(part, state).map(ReasoningPart::Visible)
                    }
                    PartKind::ReasoningSummary => {
                        bounded_text(part, state).map(ReasoningPart::Summary)
                    }
                    PartKind::Opaque => opaque_reasoning(part),
                    PartKind::Text | PartKind::ToolArguments => Err(MaterializeError::InvalidState),
                })
                .collect::<Result<Vec<_>, _>>()?;
            ReasoningItem::new(
                item.identity.id().clone(),
                reasoning,
                item.identity.wire_identity().cloned(),
            )
            .map(OutputItem::Reasoning)
            .map_err(|_| MaterializeError::InvalidValue)
        }
        ItemHeader::ToolCall { call, tool } => {
            let [part] = parts.as_slice() else {
                return Err(MaterializeError::InvalidState);
            };
            if part.kind != PartKind::ToolArguments {
                return Err(MaterializeError::InvalidState);
            }
            let arguments = part
                .parsed_arguments
                .clone()
                .ok_or(MaterializeError::InvalidState)?;
            Ok(OutputItem::ToolCall(ToolCall::new(
                item.identity.id().clone(),
                call.clone(),
                tool.clone(),
                ToolInput::Function(arguments),
                item.identity.wire_identity().cloned(),
            )))
        }
    }
}

fn bounded_text(part: &PartBuilder, state: &EventState) -> Result<TextValue, MaterializeError> {
    TextValue::new(part.value.clone(), state.limits.max_part_bytes())
        .map_err(|_| MaterializeError::InvalidValue)
}

fn opaque_reasoning(part: &PartBuilder) -> Result<ReasoningPart, MaterializeError> {
    let namespace_value = "openai.responses";
    let namespace = ProviderNamespace::new(namespace_value, namespace_value.len())
        .map_err(|_| MaterializeError::InvalidValue)?;
    let payload = part.opaque.clone().ok_or(MaterializeError::InvalidState)?;
    OpaqueState::new(
        namespace,
        OpaqueKind::EncryptedContent,
        payload,
        None,
        OpaqueExposure::InternalOnly,
    )
    .map(ReasoningPart::Opaque)
    .map_err(|_| MaterializeError::InvalidValue)
}
