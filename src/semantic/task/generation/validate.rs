//! Pure validation, shared by request transforms, response construction and lowering.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_ITEMS: usize = 1024;
pub const MAX_TEXT_BYTES: usize = 1 << 20;
pub const MAX_TOTAL_BYTES: usize = 4 << 20;
pub const MAX_TOOLS: usize = 128;

pub fn items(items: &[(ItemId, Item)], response: bool) -> Result<(), GenerationError> {
    if items.is_empty() {
        return Err(GenerationError::EmptyInput);
    }
    if items.len() > MAX_ITEMS {
        return Err(GenerationError::Limit);
    }
    let mut ids = BTreeSet::new();
    let mut parts = BTreeSet::new();
    let mut calls = BTreeSet::new();
    let mut results = BTreeSet::new();
    let mut owners = BTreeMap::new();
    let mut active_owner = None;
    let mut bytes = 0usize;
    for (id, item) in items {
        if !ids.insert(*id) {
            return Err(GenerationError::DuplicateItemId);
        }
        match item {
            Item::Instruction(i) => {
                if response {
                    return Err(GenerationError::InvalidResponse);
                }
                active_owner = None;
                add(&mut bytes, i.text.as_str())?;
            }
            Item::Message(m) => {
                if response && m.role != MessageRole::Assistant {
                    return Err(GenerationError::InvalidResponse);
                }
                active_owner = (m.role == MessageRole::Assistant).then_some(*id);
                if m.parts.len() > MAX_ITEMS {
                    return Err(GenerationError::Limit);
                }
                owners.insert(*id, (m.role, m.parts.is_empty(), false));
                for p in &m.parts {
                    if !parts.insert(p.id) {
                        return Err(GenerationError::DuplicatePartId);
                    }
                    if parts.len() > MAX_ITEMS {
                        return Err(GenerationError::Limit);
                    }
                    if let ContentPart::Text(t) = &p.content {
                        add(&mut bytes, t.as_str())?;
                    }
                }
            }
            Item::ToolCall(c) => {
                if !calls.insert(c.call_id.as_str()) {
                    return Err(GenerationError::DuplicateCall);
                }
                add(&mut bytes, c.call_id.as_str())?;
                add(&mut bytes, c.name.as_str())?;
                add(&mut bytes, &c.arguments)?;
                if c.call_id.as_str().is_empty()
                    || c.name.as_str().is_empty()
                    || c.call_id.as_str().len() > 256
                    || c.name.as_str().len() > 128
                {
                    return Err(GenerationError::Limit);
                }
                if let Some(owner) = c.message {
                    if active_owner != Some(owner) {
                        return Err(GenerationError::InvalidMessageGroup);
                    }
                    let entry = owners
                        .get_mut(&owner)
                        .ok_or(GenerationError::InvalidMessageGroup)?;
                    if entry.0 != MessageRole::Assistant {
                        return Err(GenerationError::InvalidMessageGroup);
                    }
                    entry.2 = true;
                } else {
                    active_owner = None;
                }
            }
            Item::ToolResult(r) => {
                if response {
                    return Err(GenerationError::InvalidResponse);
                }
                active_owner = None;
                if !calls.contains(r.call_id.as_str()) || !results.insert(r.call_id.as_str()) {
                    return Err(GenerationError::InvalidToolResult);
                }
                add(&mut bytes, r.call_id.as_str())?;
                add(&mut bytes, &r.output)?;
            }
        }
    }
    if owners.values().any(|(_, empty, calls)| *empty && !calls) {
        return Err(GenerationError::EmptyMessage);
    }
    Ok(())
}
fn add(total: &mut usize, value: &str) -> Result<(), GenerationError> {
    *total = total
        .checked_add(value.len())
        .ok_or(GenerationError::Limit)?;
    if value.len() > MAX_TEXT_BYTES || *total > MAX_TOTAL_BYTES {
        return Err(GenerationError::Limit);
    }
    Ok(())
}
pub fn tools(tools: &[ToolDefinition], choice: Option<&ToolChoice>) -> Result<(), GenerationError> {
    if tools.len() > MAX_TOOLS {
        return Err(GenerationError::Limit);
    }
    let mut names = BTreeSet::new();
    let mut bytes = 0;
    for ToolDefinition::Function(t) in tools {
        if !names.insert(t.name.as_str())
            || t.name.as_str().is_empty()
            || t.name.as_str().len() > 128
        {
            return Err(GenerationError::InvalidToolDefinition);
        }
        add(&mut bytes, t.name.as_str())?;
        if let Some(d) = &t.description {
            add(&mut bytes, d)?;
        }
        if let Some(schema) = &t.parameters {
            if !schema.is_object() {
                return Err(GenerationError::InvalidToolDefinition);
            }
            add(&mut bytes, &schema.to_string())?;
        }
    }
    match choice {
        Some(ToolChoice::Specific(name)) if !names.contains(name.as_str()) => {
            Err(GenerationError::InvalidToolChoice)
        }
        Some(ToolChoice::Required) if names.is_empty() => Err(GenerationError::InvalidToolChoice),
        _ => Ok(()),
    }
}
