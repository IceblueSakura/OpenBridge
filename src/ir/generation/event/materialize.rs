//! Pure materialization from Event IR state into Static Generation IR.

use thiserror::Error;

use super::algebra::*;
use crate::ir::generation::{
    Candidate, ContentPart, GenerationResponse, MessageRole, OpaqueExposure, OpaqueKind,
    OpaqueState, OutputItem, ProviderNamespace, ReasoningItem, ReasoningPart, ResponseMessage,
    ResponseStatus, TextValue, ToolCall, ToolInput,
};

/// Failure to construct one Static response from Event IR.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MaterializeError {
    #[error("event state has no terminal")]
    MissingTerminal,
    #[error("event state is incomplete or internally inconsistent")]
    InvalidState,
    #[error("event payload cannot enter the validated Static IR")]
    InvalidValue,
}

/// Materializes the terminal turn while preserving real partial output for non-completed status.
pub fn materialize(state: &EventState) -> Result<GenerationResponse, MaterializeError> {
    let terminal = state
        .terminal
        .as_ref()
        .ok_or(MaterializeError::MissingTerminal)?;
    let status = match terminal.status() {
        TerminalStatus::Completed => ResponseStatus::Completed,
        TerminalStatus::Incomplete => ResponseStatus::Incomplete,
        TerminalStatus::Failed | TerminalStatus::Error => ResponseStatus::Failed,
        TerminalStatus::Cancelled => ResponseStatus::Cancelled,
    };
    let allow_partial = status != ResponseStatus::Completed;
    let response = state
        .response
        .as_ref()
        .ok_or(MaterializeError::InvalidState)?;

    if !allow_partial
        && (state.candidates.is_empty()
            || state
                .candidates
                .values()
                .any(|candidate| !candidate.finished)
            || state.items.values().any(|item| !item.finished)
            || state.parts.values().any(|part| !part.finished))
    {
        return Err(MaterializeError::InvalidState);
    }
    let candidates = state
        .candidate_indexes
        .values()
        .map(|candidate_id| materialize_candidate(state, candidate_id, allow_partial))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GenerationResponse::new(
        response.id().clone(),
        candidates,
        status,
        state.usage,
        state.extensions.clone(),
    )
    .map_err(|_| MaterializeError::InvalidState)?
    .with_failure(terminal.failure().cloned()))
}

fn materialize_candidate(
    state: &EventState,
    candidate_id: &crate::ir::generation::CandidateId,
    allow_partial: bool,
) -> Result<Candidate, MaterializeError> {
    let candidate = state
        .candidates
        .get(candidate_id)
        .filter(|candidate| allow_partial || candidate.finished)
        .ok_or(MaterializeError::InvalidState)?;
    let output = candidate
        .items
        .values()
        .map(|item_id| materialize_item(state, item_id, allow_partial))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
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
    allow_partial: bool,
) -> Result<Option<OutputItem>, MaterializeError> {
    let item = state
        .items
        .get(item_id)
        .filter(|item| allow_partial || item.finished)
        .ok_or(MaterializeError::InvalidState)?;
    let parts = item
        .parts
        .values()
        .map(|part_id| {
            let part = state
                .parts
                .get(part_id)
                .ok_or(MaterializeError::InvalidState)?;
            if !allow_partial && !part.finished {
                return Err(MaterializeError::InvalidState);
            }
            Ok(part)
        })
        .collect::<Result<Vec<_>, MaterializeError>>()?;

    match &item.header {
        ItemHeader::Message {
            role: MessageRole::Assistant,
        } => {
            let content = parts
                .iter()
                .filter(|part| part.has_payload())
                .map(|part| {
                    let text = bounded_text(part, state)?;
                    match part.kind {
                        PartKind::Text => Ok(ContentPart::text(text)),
                        PartKind::Refusal => Ok(ContentPart::Refusal(text)),
                        _ => Err(MaterializeError::InvalidState),
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            ResponseMessage::new(
                item.identity.id().clone(),
                content,
                item.identity.wire_identity().cloned(),
            )
            .map(|message| Some(OutputItem::Message(message)))
            .map_err(|_| MaterializeError::InvalidValue)
        }
        ItemHeader::Message { .. } => Err(MaterializeError::InvalidState),
        ItemHeader::Reasoning => {
            let reasoning = parts
                .iter()
                .filter(|part| part.has_payload())
                .map(|part| match part.kind {
                    PartKind::ReasoningText => {
                        bounded_text(part, state).map(ReasoningPart::Visible)
                    }
                    PartKind::ReasoningSummary => {
                        bounded_text(part, state).map(ReasoningPart::Summary)
                    }
                    PartKind::Opaque => opaque_reasoning(part),
                    PartKind::Text | PartKind::Refusal | PartKind::ToolArguments => {
                        Err(MaterializeError::InvalidState)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            if reasoning.is_empty() {
                return Ok(None);
            }
            ReasoningItem::new(
                item.identity.id().clone(),
                reasoning,
                item.identity.wire_identity().cloned(),
            )
            .map(|item| Some(OutputItem::Reasoning(item)))
            .map_err(|_| MaterializeError::InvalidValue)
        }
        ItemHeader::ToolCall { call, tool } => {
            let [part] = parts.as_slice() else {
                return Err(MaterializeError::InvalidState);
            };
            if part.kind != PartKind::ToolArguments {
                return Err(MaterializeError::InvalidState);
            }
            let input = match part.parsed_arguments.clone() {
                Some(arguments) => ToolInput::Function(arguments),
                None if allow_partial => ToolInput::IncompleteFunction(
                    (!part.value.is_empty())
                        .then(|| bounded_text(part, state))
                        .transpose()?,
                ),
                None => return Err(MaterializeError::InvalidState),
            };
            Ok(Some(OutputItem::ToolCall(ToolCall::new(
                item.identity.id().clone(),
                call.clone(),
                tool.clone(),
                input,
                item.identity.wire_identity().cloned(),
            ))))
        }
    }
}

impl PartBuilder {
    fn has_payload(&self) -> bool {
        !self.value.is_empty() || self.opaque.is_some()
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ir::generation::{
        BoundedOpaqueJson, EventEnvelope, EventInput, EventLimits, ExtensionKind, GenerationEvent,
        OpaquePayload, ProviderExtension, ResponseId, ResponseIdentity, Sequence, TerminalStatus,
        TurnTerminal, reduce,
    };

    #[test]
    fn extensions_materialize_and_consume_turn_budget() {
        let namespace = ProviderNamespace::new("test.provider", 64).unwrap();
        let kind = ExtensionKind::new("unknown-event", 64).unwrap();
        let payload =
            OpaquePayload::Json(BoundedOpaqueJson::new(json!({"answer": 42}), 128).unwrap());
        let extension = ProviderExtension::new(namespace, kind, payload, None).unwrap();
        let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
        state = reduce(
            state,
            EventInput::Event(Box::new(EventEnvelope::new(
                Sequence::new(0),
                GenerationEvent::ResponseStarted {
                    response: ResponseIdentity::new(
                        ResponseId::new("response-extension", 64).unwrap(),
                    ),
                },
            ))),
        )
        .unwrap();
        state = reduce(
            state,
            EventInput::Event(Box::new(EventEnvelope::new(
                Sequence::new(1),
                GenerationEvent::Extension {
                    extension: extension.clone(),
                },
            ))),
        )
        .unwrap();
        state = reduce(
            state,
            EventInput::Event(Box::new(EventEnvelope::new(
                Sequence::new(2),
                GenerationEvent::Terminal {
                    terminal: TurnTerminal::new(TerminalStatus::Cancelled, None),
                },
            ))),
        )
        .unwrap();
        let response = materialize(&state).unwrap();
        assert_eq!(response.extensions(), &[extension]);
        assert_eq!(response.status(), ResponseStatus::Cancelled);
    }
}
