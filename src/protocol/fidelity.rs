//! Bounded representation records. Only surviving semantic owners may reuse them.
use super::openai::CodecError;
use crate::semantic::{
    task::generation::*,
    value::{ReplayOrigin, Text},
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

use crate::protocol::{adaptation::Adaptation, openai::Profile};
const RESPONSE_EXTRAS_BUDGET: usize = 4096;
pub(crate) const RESPONSE_EXTRA_FIELDS: &[&str] =
    &["content_filters", "frequency_penalty", "presence_penalty"];
pub(crate) fn declared_response_extras(
    profile: Profile,
    adaptation: &Adaptation,
) -> &'static [&'static str] {
    if profile == Profile::Responses && adaptation.rules.response_extras {
        RESPONSE_EXTRA_FIELDS
    } else {
        &[]
    }
}

/// Intake audit facts only; never used to restore or change semantic values.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Normalization {
    CacheWriteDefault { profile: &'static str },
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ResponseExtras {
    protocol: Profile,
    profile: &'static str,
    origin: ReplayOrigin,
    response_id: Text,
    dependency: [u8; 32],
    values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FidelityRecords {
    response_item_ids: BTreeMap<ItemId, Text>,
    cache_breakpoints: std::collections::BTreeSet<PartId>,
    input_text_forms: std::collections::BTreeSet<PartId>,
    encrypted_reasoning: BTreeMap<ItemId, (ReasoningReplay, [u8; 32])>,
    response_extras: Option<ResponseExtras>,
    routing_extras: Option<ResponseExtras>,
    normalizations: std::collections::BTreeSet<Normalization>,
}
impl FidelityRecords {
    pub fn record_input_text(&mut self, owner: PartId) -> Result<(), CodecError> {
        if self.input_text_forms.len() >= MAX_ITEMS {
            return Err(CodecError::Limit);
        }
        self.input_text_forms.insert(owner);
        Ok(())
    }
    pub fn input_text_form(&self, owner: PartId) -> bool {
        self.input_text_forms.contains(&owner)
    }
    pub fn record_cache_breakpoint(&mut self, owner: PartId) -> Result<(), CodecError> {
        if self.cache_breakpoints.len() >= MAX_ITEMS {
            return Err(CodecError::Limit);
        }
        self.cache_breakpoints.insert(owner);
        Ok(())
    }
    pub fn cache_breakpoint(&self, owner: PartId) -> bool {
        self.cache_breakpoints.contains(&owner)
    }
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
    /// Partial snapshots are checked but cannot bind response-wide facts.
    pub(crate) fn capture_response_extras(
        &mut self,
        protocol: Profile,
        adaptation: &Adaptation,
        o: &Map<String, Value>,
        semantic: Option<&GenerationResponse>,
    ) -> Result<(), CodecError> {
        let mut values = BTreeMap::new();
        let mut used = 0usize;
        for name in declared_response_extras(protocol, adaptation) {
            if let Some(value) = o.get(*name) {
                used = used.saturating_add(
                    crate::semantic::value::json_size(value, RESPONSE_EXTRAS_BUDGET)
                        .map_err(|_| CodecError::Limit)?,
                );
                if used > RESPONSE_EXTRAS_BUDGET {
                    return Err(CodecError::Limit);
                }
                values.insert((*name).into(), value.clone());
            }
        }
        if let Some(semantic) = semantic {
            self.response_extras = if values.is_empty() {
                None
            } else {
                Some(ResponseExtras {
                    protocol,
                    profile: adaptation.profile_id,
                    origin: adaptation
                        .scope
                        .clone()
                        .ok_or(CodecError::Invalid("unbound response extras"))?,
                    response_id: Text::new(
                        o.get("id")
                            .and_then(Value::as_str)
                            .ok_or(CodecError::Invalid("response identity"))?,
                        "response identity",
                        256,
                    )
                    .map_err(|_| CodecError::Limit)?,
                    dependency: response_dependency(semantic),
                    values,
                })
            };
        }
        Ok(())
    }
    pub(crate) fn projected_response_extras(
        &self,
        protocol: Profile,
        adaptation: &Adaptation,
        semantic: &GenerationResponse,
        response_id: &str,
    ) -> Option<&BTreeMap<String, Value>> {
        self.response_extras
            .as_ref()
            .filter(|r| {
                r.protocol == protocol
                    && r.profile == adaptation.profile_id
                    && Some(&r.origin) == adaptation.scope.as_ref()
                    && r.response_id.as_str() == response_id
                    && r.dependency == response_dependency(semantic)
                    && adaptation.rules.response_extras
            })
            .map(|r| &r.values)
    }
    pub(crate) fn capture_routing_extras(
        &mut self,
        protocol: Profile,
        adaptation: &Adaptation,
        values: crate::protocol::openai::adapter_shapes::Extras,
        semantic: &GenerationResponse,
        response_id: &str,
    ) -> Result<(), CodecError> {
        crate::protocol::openai::adapter_shapes::check_budget(&values)?;
        self.routing_extras = if values.is_empty() {
            None
        } else {
            if !adaptation.rules.routing_extras {
                return Err(CodecError::Unsupported("routing extras".into()));
            }
            Some(ResponseExtras {
                protocol,
                profile: adaptation.profile_id,
                origin: adaptation
                    .scope
                    .clone()
                    .ok_or(CodecError::Invalid("unbound routing extras"))?,
                response_id: Text::new(response_id, "response identity", 256)
                    .map_err(|_| CodecError::Limit)?,
                dependency: response_dependency(semantic),
                values,
            })
        };
        Ok(())
    }
    pub(crate) fn projected_routing_extras(
        &self,
        protocol: Profile,
        adaptation: &Adaptation,
        semantic: &GenerationResponse,
        response_id: &str,
    ) -> Option<&BTreeMap<String, Value>> {
        self.routing_extras
            .as_ref()
            .filter(|r| {
                adaptation.rules.routing_extras
                    && r.protocol == protocol
                    && r.profile == adaptation.profile_id
                    && Some(&r.origin) == adaptation.scope.as_ref()
                    && r.response_id.as_str() == response_id
                    && r.dependency == response_dependency(semantic)
            })
            .map(|r| &r.values)
    }
    pub fn normalizations(&self) -> &std::collections::BTreeSet<Normalization> {
        &self.normalizations
    }
    pub(crate) fn record_cache_write_default(&mut self, profile: &'static str) {
        self.normalizations
            .insert(Normalization::CacheWriteDefault { profile });
    }
    pub(crate) fn copy_response_records(&mut self, source: &Self) {
        self.response_extras = source.response_extras.clone();
        self.routing_extras = source.routing_extras.clone();
        self.normalizations = source.normalizations.clone();
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
// This in-process fingerprint is not persisted or exposed as a wire identity.
// Hashing the complete typed response conservatively invalidates extras after edits.
fn response_dependency(response: &GenerationResponse) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(format!("{response:?}").as_bytes()).into()
}

// A digest binds replay to its typed owner without retaining a second readable payload.
fn fingerprint(item: &ReasoningItem) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update([match item.status {
        ItemLifecycle::Completed => 0,
        ItemLifecycle::Incomplete => 1,
        ItemLifecycle::InProgress => 2,
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
