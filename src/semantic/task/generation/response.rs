//! Completed or partial Generation output, independent of transport success.
use super::{GenerationError, Item, ItemId, ItemLifecycle, MAX_TEXT_BYTES};
use crate::semantic::value::Text;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    Stop,
    ToolCalls,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Completed(Completion),
    Incomplete,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncompleteReason {
    MaxOutputTokens,
    ContentFilter,
    Other(Text),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseError {
    pub code: Text,
    pub message: Text,
    pub param: Option<Text>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerminalDetails {
    pub error: Option<ResponseError>,
    pub incomplete: Option<IncompleteReason>,
}
impl TerminalDetails {
    pub fn bytes(&self) -> usize {
        let error = self.error.as_ref().map_or(0, |e| {
            e.code.as_str().len()
                + e.message.as_str().len()
                + e.param.as_ref().map_or(0, |p| p.as_str().len())
        });
        error
            + match &self.incomplete {
                Some(IncompleteReason::Other(t)) => t.as_str().len(),
                Some(_) => 32,
                None => 0,
            }
    }
    pub fn validate(&self, outcome: Outcome) -> Result<(), GenerationError> {
        if self.error.is_some() && outcome != Outcome::Failed
            || self.incomplete.is_some() && outcome != Outcome::Incomplete
        {
            return Err(GenerationError::InvalidResponse);
        }
        if let Some(e) = &self.error
            && (e.code.as_str().is_empty()
                || e.code.as_str().len() > 128
                || e.message.as_str().len() > MAX_TEXT_BYTES
                || e.param.as_ref().is_some_and(|p| p.as_str().len() > 256))
        {
            return Err(GenerationError::Limit);
        }
        if let Some(IncompleteReason::Other(t)) = &self.incomplete
            && (t.as_str().is_empty() || t.as_str().len() > 128)
        {
            return Err(GenerationError::Limit);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
}
impl Usage {
    pub fn validate(self) -> Result<(), GenerationError> {
        if self.input_tokens.checked_add(self.output_tokens) != Some(self.total_tokens)
            || self
                .reasoning_tokens
                .is_some_and(|n| n > self.output_tokens)
            || self
                .cached_input_tokens
                .is_some_and(|n| n > self.input_tokens)
        {
            return Err(GenerationError::InvalidResponse);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationResponse {
    items: Vec<(ItemId, Item)>,
    outcome: Outcome,
    usage: Option<Usage>,
    details: TerminalDetails,
}
impl GenerationResponse {
    pub fn new(
        items: Vec<(ItemId, Item)>,
        completion: Completion,
    ) -> Result<Self, GenerationError> {
        Self::from_outcome(items, Outcome::Completed(completion))
    }
    pub fn unfinished(
        items: Vec<(ItemId, Item)>,
        outcome: Outcome,
    ) -> Result<Self, GenerationError> {
        if matches!(outcome, Outcome::Completed(_)) {
            return Err(GenerationError::InvalidResponse);
        }
        Self::from_outcome(items, outcome)
    }
    pub fn from_outcome(
        items: Vec<(ItemId, Item)>,
        outcome: Outcome,
    ) -> Result<Self, GenerationError> {
        if !items.is_empty() {
            super::validate::items(&items, true)?;
        }
        if let Outcome::Completed(completion) = outcome
            && (items.iter().any(|(_, i)| matches!(i, Item::ToolCall(_)))
                != (completion == Completion::ToolCalls)
                || items
                    .iter()
                    .any(|(_, i)| i.lifecycle() == Some(ItemLifecycle::Incomplete)))
        {
            return Err(GenerationError::InvalidResponse);
        }
        Ok(Self {
            items,
            outcome,
            usage: None,
            details: TerminalDetails::default(),
        })
    }
    pub fn with_usage(mut self, usage: Usage) -> Result<Self, GenerationError> {
        usage.validate()?;
        self.usage = Some(usage);
        Ok(self)
    }
    pub fn with_details(mut self, details: TerminalDetails) -> Result<Self, GenerationError> {
        details.validate(self.outcome)?;
        let bytes = if self.items.is_empty() {
            0
        } else {
            super::validate::items(&self.items, true)?
        };
        if bytes.saturating_add(details.bytes()) > super::MAX_TOTAL_BYTES {
            return Err(GenerationError::Limit);
        }
        self.details = details;
        Ok(self)
    }
    pub fn items(&self) -> &[(ItemId, Item)] {
        &self.items
    }
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }
    pub const fn completion(&self) -> Option<Completion> {
        match self.outcome {
            Outcome::Completed(c) => Some(c),
            _ => None,
        }
    }
    pub const fn usage(&self) -> Option<Usage> {
        self.usage
    }
    pub fn details(&self) -> &TerminalDetails {
        &self.details
    }
    pub fn with_items(
        self,
        items: Vec<(ItemId, Item)>,
        completion: Completion,
    ) -> Result<Self, GenerationError> {
        let mut response = Self::new(items, completion)?;
        response.usage = self.usage;
        Ok(response)
    }
}
