//! Responses reasoning controls and readable reasoning items.
//!
//! An absent object, an omitted child, and explicit `none` are distinct.
//! Opaque Responses values belong to their typed item, never to readable parts.
//! Source records bind provenance and dependencies without owning another payload.
use super::PartId;
use crate::semantic::value::Text;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningPresence {
    #[default]
    Absent,
    Null,
    Present,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ReasoningContext {
    Auto,
    CurrentTurn,
    AllTurns,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ReasoningMode {
    Standard,
    Pro,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReasoningRequest {
    pub presence: ReasoningPresence,
    pub effort: crate::semantic::value::Presence<ReasoningEffort>,
    pub summary: crate::semantic::value::Presence<ReasoningSummary>,
    pub context: crate::semantic::value::Presence<ReasoningContext>,
    pub mode: crate::semantic::value::Presence<ReasoningMode>,
    encrypted_output: bool,
}
impl ReasoningRequest {
    pub fn absent() -> Self {
        Self::default()
    }
    pub fn present(effort: Option<ReasoningEffort>, summary: Option<ReasoningSummary>) -> Self {
        use crate::semantic::value::Presence;
        Self {
            presence: ReasoningPresence::Present,
            effort: effort.map_or(Presence::Absent, Presence::Value),
            summary: summary.map_or(Presence::Absent, Presence::Value),
            ..Default::default()
        }
    }
    pub fn with_encrypted_output(mut self, enabled: bool) -> Self {
        self.encrypted_output = enabled;
        self
    }
    pub fn encrypted_output(&self) -> bool {
        self.encrypted_output
    }
    pub fn presence(&self) -> ReasoningPresence {
        self.presence
    }
    pub fn effort(&self) -> Option<ReasoningEffort> {
        self.effort.value().copied()
    }
    pub fn summary(&self) -> Option<ReasoningSummary> {
        self.summary.value().copied()
    }
    pub fn validate(&self) -> Result<(), super::GenerationError> {
        if self.presence != ReasoningPresence::Present
            && (!self.effort.is_absent()
                || !self.summary.is_absent()
                || !self.context.is_absent()
                || !self.mode.is_absent())
        {
            return Err(super::GenerationError::InvalidControl);
        }
        if self.effort() == Some(ReasoningEffort::None)
            && self
                .summary()
                .is_some_and(|s| s != ReasoningSummary::Disabled)
        {
            return Err(super::GenerationError::InvalidControl);
        }
        Ok(())
    }
}
/// Opaque Responses replay token.
///
/// SSE has no encrypted-content delta event. `output_item.added` may carry a
/// partial value, and only `output_item.done` carries the replayable token.
#[derive(Clone, Eq, PartialEq)]
pub enum EncryptedReasoning {
    /// Incomplete value from `response.output_item.added`. Not replayable.
    Partial(Text),
    /// Final value from `response.output_item.done`. This is the only replay token.
    Final(Text),
}
impl std::fmt::Debug for EncryptedReasoning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Partial(_) => "Partial([REDACTED])",
            Self::Final(_) => "Final([REDACTED])",
        })
    }
}
impl EncryptedReasoning {
    pub fn validate(&self) -> Result<(), super::GenerationError> {
        if self.as_str().is_empty() || self.as_str().len() > super::MAX_TEXT_BYTES {
            return Err(super::GenerationError::Limit);
        }
        Ok(())
    }
    // Dependency hashing must not use redacted Debug output as a value identity.
    pub(crate) fn fingerprint(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update([u8::from(matches!(self, Self::Final(_)))]);
        hash.update(self.as_str().as_bytes());
        hash.finalize().into()
    }
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
/// Event-owned value and trusted intake scope. Materialization moves the value
/// into its reasoning item; fidelity retains only a source/dependency binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningReplay {
    pub value: EncryptedReasoning,
    pub origin: Option<crate::semantic::value::ReplayOrigin>,
}
impl ReasoningReplay {
    pub fn validate(&self) -> Result<(), super::GenerationError> {
        self.value.validate()
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
    pub encrypted: Option<EncryptedReasoning>,
}
