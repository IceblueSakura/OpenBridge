//! Chat message grouping and function-history lowering from stable request identities.

use super::*;

pub(super) fn encode(
    request: &WireRequest,
    old: &GenerationRequest,
    target: &mut Map<String, Value>,
) -> Result<(), StaticCodecError> {
    let raw = request.source["messages"]
        .as_array()
        .ok_or(StaticCodecError::InvalidShape)?;
    let old_groups = groups(old);
    // A source-only empty/audio-history message is not a deletable IR item yet.
    if old_groups.len() != raw.len() {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let old_by_group: BTreeMap<_, _> = old_groups
        .iter()
        .zip(raw)
        .map(|(entries, wire)| (entries[0].0.group(), (entries, wire)))
        .collect();
    let mut messages = Vec::new();
    for entries in groups(&request.semantic) {
        let previous = old_by_group.get(&entries[0].0.group());
        if previous.is_some_and(|(old, _)| **old == entries) {
            messages.push(previous.unwrap().1.clone());
            continue;
        }
        let old_entries = previous
            .map(|(entries, _)| entries.as_slice())
            .unwrap_or(&[]);
        let raw = previous.map(|(_, raw)| *raw);
        if entries.len() == 1 {
            let (identity, item) = entries[0];
            let previous = old_entries
                .iter()
                .find(|(old, _)| old.id() == identity.id())
                .map(|(_, item)| *item);
            let source = previous.and(raw);
            match item {
                InputItem::Instruction(value) => {
                    let old = match previous {
                        Some(InputItem::Instruction(old)) => Some(old),
                        _ => None,
                    };
                    messages.push(instruction(value, old, source, true)?);
                    continue;
                }
                InputItem::ToolResult(value) => {
                    let output = tool_result(value)?;
                    if !output.is_string() {
                        return Err(StaticCodecError::UnsupportedSemantics);
                    }
                    let mut value = source
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    only_fields(&value, &["role", "content", "tool_call_id"])?;
                    let InputItem::ToolResult(result) = item else {
                        unreachable!()
                    };
                    value.insert("role".into(), json!("tool"));
                    value.insert("tool_call_id".into(), json!(result.call_id().as_str()));
                    value.insert("content".into(), output);
                    messages.push(Value::Object(value));
                    continue;
                }
                _ => {}
            }
        }
        messages.push(assistant_or_message(&entries, old_entries, raw)?);
    }
    target.insert("messages".into(), Value::Array(messages));
    Ok(())
}

fn assistant_or_message(
    entries: &[Entry<'_>],
    old: &[Entry<'_>],
    source: Option<&Value>,
) -> Result<Value, StaticCodecError> {
    let old_by_id: BTreeMap<_, _> = old
        .iter()
        .map(|(identity, item)| (identity.id(), *item))
        .collect();
    let has_binding = entries
        .iter()
        .any(|(identity, _)| old_by_id.contains_key(&identity.id()));
    let source = source.filter(|_| has_binding);
    let mut base = source
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    only_fields(
        &base,
        &[
            "role",
            "content",
            "refusal",
            "tool_calls",
            "reasoning_content",
        ],
    )?;
    let mut reasoning = None;
    let mut message_entry = None;
    let mut calls = Vec::new();
    for (position, (identity, item)) in entries.iter().enumerate() {
        match item {
            InputItem::ReasoningReplay(value) if position == 0 => {
                if old_by_id.get(&identity.id()).copied() != Some(*item) {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                reasoning = base.get("reasoning_content").cloned();
                if reasoning.is_none() || value.parts().is_empty() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
            }
            InputItem::Message(value) if message_entry.is_none() && calls.is_empty() => {
                message_entry = Some((identity, value))
            }
            InputItem::PriorToolCall(value) if message_entry.is_none() => {
                calls.push((identity, value))
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    let old_calls: Vec<_> = old
        .iter()
        .filter_map(|(identity, item)| match item {
            InputItem::PriorToolCall(call) => Some((identity.id(), call)),
            _ => None,
        })
        .collect();
    let raw_calls = base
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if has_binding && old_calls.len() != raw_calls.len() {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let old_calls: BTreeMap<_, _> = old_calls
        .into_iter()
        .zip(&raw_calls)
        .map(|((id, call), raw)| (id, (call, raw)))
        .collect();
    base.remove("reasoning_content");
    base.remove("tool_calls");
    let mut encoded = if let Some((identity, message)) = message_entry {
        let original = match old_by_id.get(&identity.id()).copied() {
            Some(InputItem::Message(old)) => Some(old),
            _ => None,
        };
        let raw = Value::Object(base.clone());
        let raw = original.map(|_| &raw);
        let mut value = super::message(message, original, raw, true)?
            .as_object()
            .cloned()
            .ok_or(StaticCodecError::InvalidShape)?;
        if reasoning.is_some() && message.role() != MessageRole::Assistant {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        value.remove("reasoning_content");
        value
    } else {
        // A calls-only message owns its null content; removed text cannot survive in the envelope.
        base.remove("refusal");
        base.insert("role".into(), json!("assistant"));
        base.insert("content".into(), Value::Null);
        base
    };
    if !calls.is_empty() {
        let mut values = Vec::new();
        for (identity, call) in calls {
            let previous = old_calls.get(&identity.id());
            values.push(native_call(
                call,
                previous.map(|(call, _)| *call),
                previous.map(|(_, wire)| *wire),
                true,
            )?);
        }
        encoded.insert("tool_calls".into(), Value::Array(values));
    }
    if let Some(value) = reasoning {
        encoded.insert("reasoning_content".into(), value);
    }
    Ok(Value::Object(encoded))
}
