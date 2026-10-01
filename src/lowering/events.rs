//! Incremental representability checks against one immutable output target.
use super::generation::{GenerationRepresentationContract, RepresentationError};
use crate::{protocol::openai::Profile, semantic::task::generation::*};

pub fn check_event(
    state: &StreamState,
    event: &StreamEvent,
    profile: Profile,
    contract: &GenerationRepresentationContract,
) -> Result<(), RepresentationError> {
    match event {
        StreamEvent::Usage(usage) => {
            super::generation::check_usage(*usage, profile, &contract.adaptation.rules)?
        }
        StreamEvent::ItemStarted { kind, replay, .. } => {
            match kind {
                // Reject before rendering the call: a terminal check cannot
                // repair membership lost from an already delivered item.
                ItemKind::ToolCall {
                    message: Some(_), ..
                } if profile == Profile::Responses => {
                    return Err(RepresentationError::MessageGrouping);
                }
                ItemKind::Message { phase: Some(_) } if profile == Profile::Chat => {
                    return Err(RepresentationError::UnmigratedSemantic);
                }
                ItemKind::ToolCall { context, .. }
                    if profile == Profile::Chat && !context.is_direct() =>
                {
                    return Err(RepresentationError::Tools);
                }
                ItemKind::ConfigurationUpdate { .. }
                    if profile != Profile::Responses || !contract.semantics.reasoning =>
                {
                    return Err(RepresentationError::Reasoning);
                }
                // Chat reasoning is provisional at event time: its carrier
                // message and single text part are validated at the terminal.
                ItemKind::Reasoning
                    if !contract.semantics.reasoning
                        || (profile == Profile::Chat
                            && !contract.adaptation.rules.readable_reasoning) =>
                {
                    return Err(RepresentationError::Reasoning);
                }
                ItemKind::CustomCall { .. }
                    if profile != Profile::Responses
                        || !contract.semantics.tools
                        || !contract.semantics.custom_tools =>
                {
                    return Err(RepresentationError::Tools);
                }
                ItemKind::Program { .. } | ItemKind::ProgramOutput { .. }
                    if profile != Profile::Responses || !contract.semantics.tools =>
                {
                    return Err(RepresentationError::Tools);
                }
                ItemKind::ToolCall { .. } if !contract.semantics.tools => {
                    return Err(RepresentationError::Tools);
                }
                ItemKind::Message { .. }
                    if profile == Profile::Chat
                        && state.items().iter().any(|i| {
                            matches!(
                                i.kind,
                                ItemKind::Message { .. } | ItemKind::ToolCall { message: None, .. }
                            )
                        }) =>
                {
                    return Err(RepresentationError::MessageGrouping);
                }
                ItemKind::ToolCall { message, .. }
                    if profile == Profile::Chat
                        && state.items().iter().any(|i| {
                            matches!(i.kind, ItemKind::Message { .. }) && Some(i.id) != *message
                        }) =>
                {
                    return Err(RepresentationError::MessageGrouping);
                }
                _ => {}
            }
            if replay
                .as_ref()
                .is_some_and(|r| !r.permits(contract.replay_origin.as_ref()))
            {
                return Err(RepresentationError::ReplayOrigin);
            }
        }
        StreamEvent::ItemFinished { replay, .. }
            if replay.as_ref().is_some_and(|r| {
                !r.permits(contract.replay_origin.as_ref())
                    || profile == Profile::Chat && r.value.replay_token().is_none()
            }) =>
        {
            return Err(RepresentationError::ReplayOrigin);
        }
        StreamEvent::ItemFinished {
            status,
            replay: Some(_),
            ..
        } if profile == Profile::Chat && *status != ItemLifecycle::Completed => {
            return Err(RepresentationError::Terminal);
        }
        StreamEvent::AnnotationAdded { .. }
            if profile == Profile::Chat || !contract.semantics.text_metadata =>
        {
            return Err(RepresentationError::TextMetadata);
        }
        StreamEvent::Queued if profile != Profile::Responses => {
            return Err(RepresentationError::Lifecycle);
        }
        StreamEvent::Delta {
            item,
            part,
            logprobs,
            ..
        } if !logprobs.is_empty() => {
            probability_target(state, *item, *part, logprobs, profile, contract)?;
        }
        StreamEvent::LogprobsSnapshot {
            item,
            part,
            logprobs,
        } => {
            probability_target(state, *item, *part, logprobs, profile, contract)?;
            if profile == Profile::Chat {
                let old = &state.part(*item, *part)?.logprobs;
                if let Some(old) = old.value()
                    && old != logprobs
                {
                    return Err(RepresentationError::TextMetadata);
                }
            }
        }
        StreamEvent::TextMetadata {
            item,
            part,
            annotations,
            logprobs,
        } => {
            if !annotations.is_empty()
                && (profile == Profile::Chat || !contract.semantics.text_metadata)
            {
                return Err(RepresentationError::TextMetadata);
            }
            if !logprobs.is_absent() {
                probability_target(
                    state,
                    *item,
                    *part,
                    logprobs.value().map(Vec::as_slice).unwrap_or_default(),
                    profile,
                    contract,
                )?;
            }
            if profile == Profile::Chat
                && let Some(old) = state.part(*item, *part)?.logprobs.value()
                && logprobs.value() != Some(old)
            {
                // A final snapshot cannot retract or rewrite facts already emitted to Chat.
                return Err(RepresentationError::TextMetadata);
            }
        }
        StreamEvent::PartStarted { item, .. }
            if profile == Profile::Chat
                && matches!(state.item(*item)?.kind, ItemKind::Message { .. })
                && !state.item(*item)?.parts.is_empty() =>
        {
            return Err(RepresentationError::MessageGrouping);
        }
        StreamEvent::Terminal { terminal, details }
            if profile == Profile::Chat
                && (*terminal == StreamTerminal::Incomplete
                    && !matches!(
                        details.incomplete,
                        Some(IncompleteReason::MaxOutputTokens | IncompleteReason::ContentFilter)
                    )
                    || matches!(
                        terminal,
                        StreamTerminal::Failed | StreamTerminal::Cancelled | StreamTerminal::Error
                    )
                    || details.incomplete.as_ref().is_some_and(|r| {
                        !matches!(
                            r,
                            IncompleteReason::MaxOutputTokens | IncompleteReason::ContentFilter
                        )
                    })) =>
        {
            return Err(RepresentationError::Terminal);
        }
        _ => {}
    }
    Ok(())
}
fn probability_target(
    state: &StreamState,
    item: ItemId,
    part: PartId,
    logprobs: &[Logprob],
    profile: Profile,
    contract: &GenerationRepresentationContract,
) -> Result<(), RepresentationError> {
    if !contract.semantics.logprobs
        || !contract.semantics.text_metadata
        || profile == Profile::Responses && state.part(item, part)?.kind == PartKind::Refusal
    {
        return Err(RepresentationError::TextMetadata);
    }
    if profile == Profile::Chat {
        crate::protocol::openai::chat_logprobs::validate(logprobs)
            .map_err(|_| RepresentationError::TextMetadata)?;
    }
    Ok(())
}
