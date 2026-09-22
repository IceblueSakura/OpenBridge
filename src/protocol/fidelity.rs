//! Bounded representation-only records. No source object can override semantic fields.
use crate::semantic::{
    task::generation::{ItemId, MAX_ITEMS},
    value::Text,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FidelityRecords {
    response_item_ids: BTreeMap<ItemId, Text>,
}
impl FidelityRecords {
    pub fn record_response_item_id(
        &mut self,
        owner: ItemId,
        value: &str,
    ) -> Result<(), super::openai::CodecError> {
        if self.response_item_ids.len() >= MAX_ITEMS
            || self
                .response_item_ids
                .values()
                .any(|id| id.as_str() == value)
        {
            return Err(super::openai::CodecError::Invalid(
                "duplicate or excessive wire item identities",
            ));
        }
        let value =
            Text::new(value, "wire item id", 256).map_err(|_| super::openai::CodecError::Limit)?;
        self.response_item_ids.insert(owner, value);
        Ok(())
    }
    pub fn response_item_id(&self, owner: ItemId) -> Option<&str> {
        self.response_item_ids.get(&owner).map(Text::as_str)
    }
}
