//! Single-candidate completed static output for the function-tool migration slice.
use super::{GenerationError, Item, ItemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    Stop,
    ToolCalls,
}
/// Terminal outcome. Non-completed values retain partial output without becoming success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Completed(Completion),
    Incomplete,
    Failed,
}
/// Provider-reported usage. Absence is distinct from zero, and totals are never estimated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationResponse {
    items: Vec<(ItemId, Item)>,
    outcome: Outcome,
    usage: Option<Usage>,
}
impl GenerationResponse {
    pub fn new(
        items: Vec<(ItemId, Item)>,
        completion: Completion,
    ) -> Result<Self, GenerationError> {
        Self::from_outcome(items, Outcome::Completed(completion), None)
    }
    /// Retains partial text or arguments. This is not a successful completion.
    pub fn unfinished(
        items: Vec<(ItemId, Item)>,
        outcome: Outcome,
    ) -> Result<Self, GenerationError> {
        if matches!(outcome, Outcome::Completed(_)) {
            return Err(GenerationError::InvalidResponse);
        }
        Self::from_outcome(items, outcome, None)
    }
    pub fn with_usage(mut self, usage: Usage) -> Result<Self, GenerationError> {
        if usage.input_tokens.checked_add(usage.output_tokens) != Some(usage.total_tokens) {
            return Err(GenerationError::InvalidResponse);
        }
        self.usage = Some(usage);
        Ok(self)
    }
    fn from_outcome(
        items: Vec<(ItemId, Item)>,
        outcome: Outcome,
        usage: Option<Usage>,
    ) -> Result<Self, GenerationError> {
        if items.is_empty() {
            if matches!(outcome, Outcome::Completed(_)) {
                return Err(GenerationError::EmptyInput);
            }
        } else {
            super::validate::items(&items, true)?;
        }
        if let Outcome::Completed(completion) = outcome {
            let has_calls = items.iter().any(|(_, i)| matches!(i, Item::ToolCall(_)));
            if has_calls != (completion == Completion::ToolCalls) {
                return Err(GenerationError::InvalidResponse);
            }
        }
        Ok(Self {
            items,
            outcome,
            usage,
        })
    }
    pub fn items(&self) -> &[(ItemId, Item)] {
        &self.items
    }
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }
    pub const fn completion(&self) -> Option<Completion> {
        match self.outcome {
            Outcome::Completed(completion) => Some(completion),
            Outcome::Incomplete | Outcome::Failed => None,
        }
    }
    pub const fn usage(&self) -> Option<Usage> {
        self.usage
    }
    pub fn with_items(
        self,
        items: Vec<(ItemId, Item)>,
        completion: Completion,
    ) -> Result<Self, GenerationError> {
        let usage = self.usage;
        let mut response = Self::new(items, completion)?;
        response.usage = usage;
        Ok(response)
    }
}
