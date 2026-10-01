//! Derived continuation facts for the admitted client-call slice, never execution policy.
use super::{GenerationResponse, Item, ItemId, Outcome};

/// Local owner and wire call identity are distinct; positions are not references.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallReference<'a> {
    pub item: ItemId,
    pub call_id: &'a str,
}

/// Requirements derivable from a complete response's final typed content.
/// This view neither validates tool arguments nor authorizes tool execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Continuation<'a> {
    /// No supported continuation fact is known. This never asserts end-of-turn.
    Unreported,
    /// Nonempty, ordered references to calls without results in this response.
    ToolResults(Vec<CallReference<'a>>),
}

impl GenerationResponse {
    /// Recompute after edits instead of retaining stale call IDs or a finish label.
    pub fn continuation(&self) -> Continuation<'_> {
        if self.outcome() != Outcome::Completed {
            return Continuation::Unreported;
        }
        let calls: Vec<_> = self
            .items()
            .iter()
            .enumerate()
            .filter_map(|(index, (item, value))| {
                let call_id = match value {
                    Item::ToolCall(call) => &call.call_id,
                    Item::CustomCall(call) => &call.call_id,
                    Item::Program(call) => {
                        // A program with a reported result is not an outstanding call.
                        // Keep the ordered association; an earlier output cannot resolve it.
                        if self.items()[index + 1..].iter().any(|(_, value)| {
                            matches!(value, Item::ProgramOutput(result) if result.call_id == call.call_id)
                        }) {
                            return None;
                        }
                        &call.call_id
                    }
                    _ => return None,
                };
                Some(CallReference {
                    item: *item,
                    call_id: call_id.as_str(),
                })
            })
            .collect();
        if calls.is_empty() {
            Continuation::Unreported
        } else {
            Continuation::ToolResults(calls)
        }
    }
}
