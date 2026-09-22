//! Single-candidate completed static output for the function-tool migration slice.
use super::{GenerationError, Item, ItemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    Stop,
    ToolCalls,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationResponse {
    items: Vec<(ItemId, Item)>,
    completion: Completion,
}
impl GenerationResponse {
    pub fn new(
        items: Vec<(ItemId, Item)>,
        completion: Completion,
    ) -> Result<Self, GenerationError> {
        super::validate::items(&items, true)?;
        let has_calls = items.iter().any(|(_, i)| matches!(i, Item::ToolCall(_)));
        if has_calls != (completion == Completion::ToolCalls) {
            return Err(GenerationError::InvalidResponse);
        }
        Ok(Self { items, completion })
    }
    pub fn items(&self) -> &[(ItemId, Item)] {
        &self.items
    }
    pub const fn completion(&self) -> Completion {
        self.completion
    }
    pub fn with_items(
        self,
        items: Vec<(ItemId, Item)>,
        completion: Completion,
    ) -> Result<Self, GenerationError> {
        Self::new(items, completion)
    }
}
