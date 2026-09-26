//! Pure validation and bounded accounting shared by construction, transforms and lowering.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
pub const MAX_ITEMS: usize = 1024;
pub const MAX_TEXT_BYTES: usize = 1 << 20;
pub const MAX_TOTAL_BYTES: usize = 4 << 20;
pub const MAX_TOOLS: usize = 128;
fn add(total: &mut usize, value: &str) -> Result<(), GenerationError> {
    charge(total, value.len())
}
fn charge(total: &mut usize, n: usize) -> Result<(), GenerationError> {
    if n > MAX_TEXT_BYTES {
        return Err(GenerationError::Limit);
    }
    *total = total.checked_add(n).ok_or(GenerationError::Limit)?;
    if *total > MAX_TOTAL_BYTES {
        return Err(GenerationError::Limit);
    }
    Ok(())
}
fn part_id(parts: &mut BTreeSet<PartId>, id: PartId) -> Result<(), GenerationError> {
    if !parts.insert(id) {
        return Err(GenerationError::DuplicatePartId);
    }
    if parts.len() > MAX_ITEMS {
        return Err(GenerationError::Limit);
    }
    Ok(())
}
pub fn items(items: &[(ItemId, Item)], response: bool) -> Result<usize, GenerationError> {
    if items.is_empty() {
        return Err(GenerationError::EmptyInput);
    }
    if items.len() > MAX_ITEMS {
        return Err(GenerationError::Limit);
    }
    let mut ids = BTreeSet::new();
    let mut parts = BTreeSet::new();
    let mut calls = BTreeMap::new();
    let mut results = BTreeSet::new();
    let mut active_owner = None;
    let mut bytes = 0;
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
                for (id, t) in &i.parts {
                    part_id(&mut parts, *id)?;
                    add(&mut bytes, t.as_str())?;
                }
                if i.parts.is_empty() {
                    return Err(GenerationError::EmptyMessage);
                }
            }
            Item::Message(m) => {
                if response && m.role != MessageRole::Assistant {
                    return Err(GenerationError::InvalidResponse);
                }
                if m.phase.is_some() && m.role != MessageRole::Assistant {
                    return Err(GenerationError::PhaseInUserMessage);
                }
                if m.role == MessageRole::User && m.parts.is_empty() {
                    return Err(GenerationError::EmptyMessage);
                }
                active_owner = (m.role == MessageRole::Assistant).then_some(*id);
                for p in &m.parts {
                    part_id(&mut parts, p.id)?;
                    match &p.content {
                        ContentPart::Text(t) => {
                            t.validate()?;
                            charge(&mut bytes, t.bytes())?;
                        }
                        ContentPart::Refusal(t) => {
                            if m.role != MessageRole::Assistant {
                                return Err(GenerationError::RefusalInUserMessage);
                            }
                            add(&mut bytes, t.as_str())?;
                        }
                        ContentPart::Resource(_) => {}
                    }
                }
            }
            Item::ToolCall(c) => {
                validate_call(
                    &mut calls,
                    &mut bytes,
                    &c.call_id,
                    &c.name,
                    &c.arguments,
                    ToolKind::Function,
                )?;
                if let Some(owner) = c.message {
                    if active_owner != Some(owner) {
                        return Err(GenerationError::InvalidMessageGroup);
                    }
                    if items.iter().any(|(id,item)|*id==owner && matches!(item,Item::Message(m) if m.parts.iter().any(|p|matches!(p.content,ContentPart::Refusal(_))))){return Err(GenerationError::InvalidResponse);}
                } else {
                    active_owner = None;
                }
            }
            Item::CustomCall(c) => {
                active_owner = None;
                validate_call(
                    &mut calls,
                    &mut bytes,
                    &c.call_id,
                    &c.name,
                    &c.input,
                    ToolKind::Custom,
                )?;
            }
            Item::Reasoning(r) => {
                active_owner = None;
                for (id, p) in &r.parts {
                    part_id(&mut parts, *id)?;
                    let (ReasoningContent::Summary(t) | ReasoningContent::Text(t)) = p;
                    add(&mut bytes, t.as_str())?;
                }
            }
            Item::ToolResult(r) | Item::CustomResult(r) => {
                if response {
                    return Err(GenerationError::InvalidResponse);
                }
                active_owner = None;
                let kind = if matches!(item, Item::CustomResult(_)) {
                    ToolKind::Custom
                } else {
                    ToolKind::Function
                };
                if calls.get(r.call_id.as_str()) != Some(&kind)
                    || !results.insert(r.call_id.as_str())
                {
                    return Err(GenerationError::InvalidToolResult);
                }
                add(&mut bytes, r.call_id.as_str())?;
                match &r.output {
                    ToolOutput::Text(t) => add(&mut bytes, t)?,
                    ToolOutput::Parts(p) => {
                        for (id, t) in p {
                            part_id(&mut parts, *id)?;
                            add(&mut bytes, t.as_str())?;
                        }
                    }
                }
            }
        }
    }
    Ok(bytes)
}
fn validate_call<'a>(
    calls: &mut BTreeMap<&'a str, ToolKind>,
    bytes: &mut usize,
    id: &'a crate::semantic::value::Text,
    name: &crate::semantic::value::Text,
    payload: &str,
    kind: ToolKind,
) -> Result<(), GenerationError> {
    if calls.insert(id.as_str(), kind).is_some() {
        return Err(GenerationError::DuplicateCall);
    }
    if id.as_str().is_empty()
        || id.as_str().len() > 256
        || name.as_str().is_empty()
        || name.as_str().len() > 128
    {
        return Err(GenerationError::Limit);
    }
    add(bytes, id.as_str())?;
    add(bytes, name.as_str())?;
    add(bytes, payload)
}
pub fn output(value: &OutputConstraint) -> Result<usize, GenerationError> {
    match value {
        OutputConstraint::Text | OutputConstraint::JsonObject => Ok(0),
        OutputConstraint::JsonSchema {
            name,
            description,
            schema: s,
            strict,
        } => {
            if name.as_str().is_empty()
                || name.as_str().len() > 64
                || !name
                    .as_str()
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(GenerationError::InvalidControl);
            }
            let mode = if *strict == Some(true) {
                super::schema::Mode::Explicit
            } else {
                super::schema::Mode::General
            };
            let mut bytes = super::schema::validate(s, mode)?;
            add(&mut bytes, name.as_str())?;
            if let Some(d) = description {
                add(&mut bytes, d.as_str())?;
            }
            Ok(bytes)
        }
    }
}
pub fn tools(
    tools: &[ToolDefinition],
    choice: Option<&ToolChoice>,
) -> Result<usize, GenerationError> {
    if tools.len() > MAX_TOOLS {
        return Err(GenerationError::Limit);
    }
    let mut available = BTreeSet::new();
    let mut bytes = 0;
    for t in tools {
        let n = t.name().as_str();
        if n.is_empty() || n.len() > 128 || !available.insert((t.kind(), n)) {
            return Err(GenerationError::InvalidToolDefinition);
        }
        add(&mut bytes, n)?;
        match t {
            ToolDefinition::Function(t) => {
                if let Some(d) = &t.description {
                    add(&mut bytes, d)?;
                }
                if let Some(s) = &t.parameters {
                    let mode = match t.strict {
                        FunctionStrictness::Explicit(true) => super::schema::Mode::Explicit,
                        FunctionStrictness::Omitted(StrictDefault::NormalizeSchema) => {
                            super::schema::Mode::Normalize
                        }
                        _ => super::schema::Mode::General,
                    };
                    charge(&mut bytes, super::schema::validate(s, mode)?)?;
                }
                if let Some(s) = &t.output_schema {
                    charge(
                        &mut bytes,
                        super::schema::validate(s, super::schema::Mode::General)?,
                    )?;
                }
            }
            ToolDefinition::Custom(t) => {
                if let Some(d) = &t.description {
                    add(&mut bytes, d)?;
                }
                if let Some(CustomFormat::Grammar { definition, .. }) = &t.format {
                    add(&mut bytes, definition.as_str())?;
                }
            }
        }
    }
    match choice {
        Some(ToolChoice::Specific(n)) if !available.contains(&(ToolKind::Function, n.as_str())) => {
            return Err(GenerationError::InvalidToolChoice);
        }
        Some(ToolChoice::Custom(n)) if !available.contains(&(ToolKind::Custom, n.as_str())) => {
            return Err(GenerationError::InvalidToolChoice);
        }
        Some(ToolChoice::Required) if tools.is_empty() => {
            return Err(GenerationError::InvalidToolChoice);
        }
        Some(ToolChoice::Allowed { tools, .. }) => {
            if tools.is_empty() || tools.len() > MAX_TOOLS {
                return Err(GenerationError::InvalidToolChoice);
            }
            let mut seen = BTreeSet::new();
            for r in tools {
                if !available.contains(&(r.kind, r.name.as_str()))
                    || !seen.insert((r.kind, r.name.as_str()))
                {
                    return Err(GenerationError::InvalidToolChoice);
                }
            }
        }
        _ => {}
    }
    Ok(bytes)
}
