//! Bounded representation records. Only surviving semantic owners may reuse them.
use super::openai::CodecError;
use crate::semantic::{
    task::generation::*,
    value::{ReplayOrigin, Text},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FidelityRecords {
    response_item_ids: BTreeMap<ItemId, Text>,
    encrypted_reasoning: BTreeMap<ItemId, (ReasoningReplay, [u8; 32])>,
}
impl FidelityRecords {
    pub fn record_response_item_id(
        &mut self,
        owner: ItemId,
        value: &str,
    ) -> Result<(), CodecError> {
        if (!self.response_item_ids.contains_key(&owner)
            && self.response_item_ids.len() >= MAX_ITEMS)
            || self
                .response_item_ids
                .iter()
                .any(|(id, text)| *id != owner && text.as_str() == value)
        {
            return Err(CodecError::Invalid(
                "duplicate or excessive wire item identities",
            ));
        }
        let value = Text::new(value, "wire item id", 256).map_err(|_| CodecError::Limit)?;
        self.response_item_ids.insert(owner, value);
        Ok(())
    }
    pub fn response_item_id(&self, owner: ItemId) -> Option<&str> {
        self.response_item_ids.get(&owner).map(Text::as_str)
    }
    pub fn record_replay(
        &mut self,
        owner: ItemId,
        value: ReasoningReplay,
        semantic: &ReasoningItem,
    ) -> Result<(), CodecError> {
        value.validate()?;
        if !self.encrypted_reasoning.contains_key(&owner)
            && self.encrypted_reasoning.len() >= MAX_ITEMS
        {
            return Err(CodecError::Limit);
        }
        if self
            .encrypted_reasoning
            .get(&owner)
            .is_some_and(|(old, _)| {
                old.value.replay_token().is_some() && value.value.replay_token().is_none()
            })
        {
            return Err(CodecError::Invalid("encrypted reasoning phase"));
        }
        let bytes: usize = self
            .encrypted_reasoning
            .iter()
            .filter(|(id, _)| **id != owner)
            .map(|(_, (r, _))| r.value.as_str().len())
            .sum();
        if bytes.saturating_add(value.value.as_str().len()) > MAX_TOTAL_BYTES {
            return Err(CodecError::Limit);
        }
        self.encrypted_reasoning
            .insert(owner, (value, fingerprint(semantic)));
        Ok(())
    }
    pub fn replay(&self, owner: ItemId) -> Option<&ReasoningReplay> {
        self.encrypted_reasoning.get(&owner).map(|(r, _)| r)
    }
    pub fn replay_matches(&self, owner: ItemId, semantic: &ReasoningItem) -> bool {
        self.encrypted_reasoning
            .get(&owner)
            .is_none_or(|(_, binding)| *binding == fingerprint(semantic))
    }
    pub fn remove_replay(&mut self, owner: ItemId) {
        self.encrypted_reasoning.remove(&owner);
    }
    pub fn encrypted_reasoning_replay(&self, owner: ItemId) -> Option<&str> {
        self.replay(owner).and_then(|r| r.value.replay_token())
    }
    /// Bind origin only at a trusted decode boundary, never from business JSON.
    pub fn bind_replay_origin(&mut self, origin: &ReplayOrigin) -> Result<(), CodecError> {
        if self
            .encrypted_reasoning
            .values()
            .any(|(r, _)| r.origin.as_ref().is_some_and(|old| old != origin))
        {
            return Err(CodecError::Invalid("replay origin rebinding"));
        }
        for (replay, _) in self.encrypted_reasoning.values_mut() {
            replay.origin = Some(origin.clone());
        }
        Ok(())
    }
    pub fn retain_owners(&mut self, items: &[(ItemId, Item)]) {
        self.response_item_ids
            .retain(|id, _| items.iter().any(|(owner, _)| owner == id));
        self.encrypted_reasoning.retain(|id, _| {
            items
                .iter()
                .any(|(owner, item)| owner == id && matches!(item, Item::Reasoning(_)))
        });
    }
}
// A digest binds replay to its typed owner without retaining a second readable payload.
fn fingerprint(item: &ReasoningItem) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update([match item.status {
        ItemLifecycle::Completed => 0,
        ItemLifecycle::Incomplete => 1,
    }]);
    for (id, part) in &item.parts {
        hash.update(id.get().to_le_bytes());
        let (kind, text) = match part {
            ReasoningContent::Summary(t) => (0, t),
            ReasoningContent::Text(t) => (1, t),
        };
        hash.update([kind]);
        hash.update((text.as_str().len() as u64).to_le_bytes());
        hash.update(text.as_str().as_bytes());
    }
    hash.finalize().into()
}
