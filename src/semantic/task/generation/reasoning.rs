//! Responses reasoning controls and readable reasoning items.
//!
//! An absent object, an omitted child, and explicit `none` are distinct.
//! Opaque encrypted replay stays in fidelity records, not in these parts.
use super::PartId;
use crate::semantic::value::Text;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningPresence {
    #[default]
    Absent,
    Present,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningSummary {
    Disabled,
    Auto,
    Concise,
    Detailed,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReasoningRequest {
    presence: ReasoningPresence,
    effort: Option<ReasoningEffort>,
    summary: Option<ReasoningSummary>,
    encrypted_output: bool,
}
impl ReasoningRequest {
    pub const fn absent() -> Self {
        Self {
            presence: ReasoningPresence::Absent,
            effort: None,
            summary: None,
            encrypted_output: false,
        }
    }
    pub const fn present(
        effort: Option<ReasoningEffort>,
        summary: Option<ReasoningSummary>,
    ) -> Self {
        Self {
            presence: ReasoningPresence::Present,
            effort,
            summary,
            encrypted_output: false,
        }
    }
    pub fn with_encrypted_output(mut self, enabled: bool) -> Self {
        self.encrypted_output = enabled;
        self
    }
    pub const fn encrypted_output(self) -> bool {
        self.encrypted_output
    }
    pub const fn presence(self) -> ReasoningPresence {
        self.presence
    }
    pub const fn effort(self) -> Option<ReasoningEffort> {
        self.effort
    }
    pub const fn summary(self) -> Option<ReasoningSummary> {
        self.summary
    }
}
/// Opaque Responses replay token.
///
/// SSE has no encrypted-content delta event. `output_item.added` may carry a
/// partial value, and only `output_item.done` carries the replayable token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncryptedReasoning {
    /// Incomplete value from `response.output_item.added`. Not replayable.
    Partial(Text),
    /// Final value from `response.output_item.done`. This is the only replay token.
    Final(Text),
}
impl EncryptedReasoning {
    pub fn replay_token(&self) -> Option<&str> {
        match self {
            Self::Partial(_) => None,
            Self::Final(value) => Some(value.as_str()),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Self::Partial(value) | Self::Final(value) => value.as_str(),
        }
    }
}
/// Representation-side replay state. It is never a readable reasoning part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningReplay {
    pub value: EncryptedReasoning,
    pub origin: Option<crate::semantic::value::ReplayOrigin>,
}
impl ReasoningReplay {
    pub fn validate(&self) -> Result<(), super::GenerationError> {
        if self.value.as_str().is_empty() || self.value.as_str().len() > super::MAX_TEXT_BYTES {
            return Err(super::GenerationError::Limit);
        }
        Ok(())
    }
    pub fn permits(&self, target: Option<&crate::semantic::value::ReplayOrigin>) -> bool {
        self.origin.is_some() && self.origin.as_ref() == target
    }
}
/// Readable reasoning content. This is not assistant message text or encrypted replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReasoningContent {
    Summary(Text),
    Text(Text),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningItem {
    pub parts: Vec<(PartId, ReasoningContent)>,
    pub status: super::ItemLifecycle,
}
