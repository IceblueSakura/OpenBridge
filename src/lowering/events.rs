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
        StreamEvent::ItemStarted { kind, replay, .. } => {
            match kind {
                ItemKind::Reasoning if profile != Profile::Responses || !contract.reasoning => {
                    return Err(RepresentationError::Reasoning);
                }
                ItemKind::ToolCall { .. } if !contract.tools => {
                    return Err(RepresentationError::Tools);
                }
                ItemKind::Message
                    if profile == Profile::Chat
                        && state.items().iter().any(|i| {
                            matches!(
                                i.kind,
                                ItemKind::Message | ItemKind::ToolCall { message: None, .. }
                            )
                        }) =>
                {
                    return Err(RepresentationError::MessageGrouping);
                }
                ItemKind::ToolCall { message, .. }
                    if profile == Profile::Chat
                        && state.items().iter().any(|i| {
                            matches!(i.kind, ItemKind::Message) && Some(i.id) != *message
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
            if replay
                .as_ref()
                .is_some_and(|r| !r.permits(contract.replay_origin.as_ref())) =>
        {
            return Err(RepresentationError::ReplayOrigin);
        }
        StreamEvent::PartStarted { item, .. }
            if profile == Profile::Chat
                && matches!(state.item(*item)?.kind, ItemKind::Message)
                && !state.item(*item)?.parts.is_empty() =>
        {
            return Err(RepresentationError::MessageGrouping);
        }
        StreamEvent::Terminal { terminal, details }
            if profile == Profile::Chat
                && (matches!(
                    terminal,
                    StreamTerminal::Failed | StreamTerminal::Cancelled | StreamTerminal::Error
                ) || details
                    .incomplete
                    .as_ref()
                    .is_some_and(|r| !matches!(r, IncompleteReason::MaxOutputTokens))) =>
        {
            return Err(RepresentationError::Terminal);
        }
        _ => {}
    }
    Ok(())
}
