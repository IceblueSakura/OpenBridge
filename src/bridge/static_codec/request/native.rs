//! Native request content ownership with stable item/group/part source bindings.
//!
//! Source spelling is usable only for the same identity and unchanged unmodeled semantics.
//! Missing mappings reject transformations instead of replaying an obsolete prompt.

use std::collections::BTreeMap;

use super::*;
use crate::ir::generation::{InputIdentity, Message, MessageRole};

mod chat;
mod responses;
#[cfg(test)]
mod tests;

type Entry<'a> = (&'a InputIdentity, &'a InputItem);

pub(super) fn encode_input(
    request: &WireRequest,
    target: &mut Map<String, Value>,
) -> Result<(), StaticCodecError> {
    let baseline = decode_request(request.protocol, &request.source, request.max_bytes)?;
    let current = &request.semantic;
    let old = &baseline.semantic;
    // Only input and already-owned sampling fields can change in this slice.
    if current.tools() != old.tools()
        || current.tool_choice() != old.tool_choice()
        || current.output() != old.output()
        || current.output_projection() != old.output_projection()
        || current.reasoning() != old.reasoning()
        || current.state() != old.state()
        || current.extensions() != old.extensions()
        || current.controls().max_output_tokens() != old.controls().max_output_tokens()
        || current.controls().parallel_tool_calls() != old.controls().parallel_tool_calls()
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    if current.input() == old.input() && current.input_identities() == old.input_identities() {
        return Ok(());
    }
    validate_calls(current.input())?;
    let old_groups: BTreeMap<_, _> = old
        .input_identities()
        .iter()
        .map(|identity| (identity.id(), identity.group()))
        .collect();
    if current.input_identities().iter().any(|identity| {
        old_groups
            .get(&identity.id())
            .is_some_and(|group| *group != identity.group())
    }) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    only_fields(
        &request.source,
        &[
            "model",
            "messages",
            "input",
            "instructions",
            "stream",
            "stream_options",
            "service_tier",
            "temperature",
            "top_p",
            "top_k",
            "seed",
            "stop",
            "n",
            "frequency_penalty",
            "presence_penalty",
            "max_tokens",
            "max_completion_tokens",
            "max_output_tokens",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "reasoning",
            "reasoning_effort",
            "response_format",
            "text",
            "include",
            "store",
            "metadata",
            "user",
            "audio",
            "modalities",
            "prompt_cache_key",
            "prompt_cache_retention",
            "safety_identifier",
            "previous_response_id",
            "conversation",
            "truncation",
            "background",
            "max_tool_calls",
        ],
    )?;
    match request.protocol {
        ApiProtocol::ChatCompletions => chat::encode(request, old, target),
        ApiProtocol::Responses => responses::encode(request, old, target),
    }
}

fn validate_calls(input: &[InputItem]) -> Result<(), StaticCodecError> {
    let mut calls = BTreeSet::new();
    let mut results = BTreeSet::new();
    for item in input {
        match item {
            InputItem::PriorToolCall(call) => {
                if !calls.insert(call.call_id()) {
                    return Err(StaticCodecError::InvalidToolIdentity);
                }
            }
            InputItem::ToolResult(result)
                if !calls.contains(result.call_id()) || !results.insert(result.call_id()) =>
            {
                return Err(StaticCodecError::InvalidToolIdentity);
            }
            _ => {}
        }
    }
    Ok(())
}

fn groups(request: &GenerationRequest) -> Vec<Vec<Entry<'_>>> {
    let mut groups: Vec<Vec<Entry<'_>>> = Vec::new();
    for entry in request.input_identities().iter().zip(request.input()) {
        if groups
            .last()
            .is_none_or(|group| group[0].0.group() != entry.0.group())
        {
            groups.push(Vec::new());
        }
        groups.last_mut().expect("group inserted").push(entry);
    }
    groups
}

fn only_fields(value: &Map<String, Value>, fields: &[&str]) -> Result<(), StaticCodecError> {
    if value.keys().any(|key| !fields.contains(&key.as_str())) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    Ok(())
}

fn tool_result(result: &ToolResult) -> Result<Value, StaticCodecError> {
    if result.status() != ToolResultStatus::Success
        || result.output().len() != 1
        || result.wire_identity().is_some()
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    match &result.output()[0] {
        ToolOutput::Text(value) => Ok(json!(value.as_str())),
        ToolOutput::Json(value) => Ok(value.as_value().clone()),
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn native_call(
    call: &ToolCall,
    original: Option<&ToolCall>,
    source: Option<&Value>,
    chat: bool,
) -> Result<Value, StaticCodecError> {
    if original == Some(call) {
        return source.cloned().ok_or(StaticCodecError::InvalidShape);
    }
    if call.wire_identity().is_some() || original.is_some_and(|old| old.id() != call.id()) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let mut encoded = source
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    only_fields(
        &encoded,
        if chat {
            &["id", "type", "function"]
        } else {
            &["id", "type", "call_id", "name", "arguments", "status"]
        },
    )?;
    let arguments = Value::Object(call.input().as_function()?.as_map().clone()).to_string();
    let old_arguments = if chat {
        encoded
            .get("function")
            .and_then(|value| value.get("arguments"))
    } else {
        encoded.get("arguments")
    };
    let arguments = if original.is_some_and(|old| old.input() == call.input()) {
        old_arguments.cloned().unwrap_or(json!(arguments))
    } else {
        json!(arguments)
    };
    if chat {
        let mut function = encoded
            .get("function")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        only_fields(&function, &["name", "arguments"])?;
        function.insert("name".into(), json!(call.tool().as_str()));
        function.insert("arguments".into(), arguments);
        encoded.insert("function".into(), Value::Object(function));
        encoded.insert("type".into(), json!("function"));
        encoded.insert("id".into(), json!(call.call_id().as_str()));
    } else {
        encoded.insert("type".into(), json!("function_call"));
        encoded.insert("call_id".into(), json!(call.call_id().as_str()));
        encoded.insert("name".into(), json!(call.tool().as_str()));
        encoded.insert("arguments".into(), arguments);
    }
    Ok(Value::Object(encoded))
}

fn message(
    current: &Message,
    original: Option<&Message>,
    source: Option<&Value>,
    chat: bool,
) -> Result<Value, StaticCodecError> {
    if original == Some(current) {
        return source.cloned().ok_or(StaticCodecError::InvalidShape);
    }
    let mut encoded = source
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    only_fields(
        &encoded,
        if chat {
            &["role", "content", "refusal"]
        } else {
            &["type", "id", "role", "content", "status"]
        },
    )?;
    let old_parts: Vec<&ContentPart> = original
        .map(|old| {
            old.content()
                .iter()
                .filter(|part| !chat || !matches!(part, ContentPart::Refusal(_)))
                .collect()
        })
        .unwrap_or_default();
    let raw_parts: Vec<Value> = match encoded.get("content") {
        Some(Value::Array(parts)) => parts.clone(),
        Some(Value::String(text)) => {
            vec![
                json!({"type": if chat { "text" } else if original.is_some_and(|old| old.role() == MessageRole::Assistant) { "output_text" } else { "input_text" }, "text":text}),
            ]
        }
        Some(Value::Null) | None => Vec::new(),
        _ => return Err(StaticCodecError::InvalidShape),
    };
    if old_parts.len() != raw_parts.len() {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let source_ids: Vec<usize> = original
        .map(|old| {
            old.part_ids()
                .iter()
                .zip(old.content())
                .filter(|(_, part)| !chat || !matches!(part, ContentPart::Refusal(_)))
                .map(|(id, _)| *id)
                .collect()
        })
        .unwrap_or_default();
    let by_id: BTreeMap<_, _> = source_ids
        .into_iter()
        .zip(old_parts.into_iter().zip(&raw_parts))
        .collect();
    let mut content = Vec::new();
    let mut refusal = None;
    for (id, part) in current.part_ids().iter().zip(current.content()) {
        if chat && let ContentPart::Refusal(value) = part {
            if refusal.replace(json!(value.as_str())).is_some() {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
            continue;
        }
        if chat && refusal.is_some() {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        let previous = by_id.get(id);
        let raw = previous.map(|(_, raw)| *raw);
        if previous.is_some_and(|(old, _)| *old == part)
            && original.is_some_and(|old| old.role() == current.role())
        {
            content.push(raw.unwrap().clone());
            continue;
        }
        let mut value = raw.and_then(Value::as_object).cloned().unwrap_or_default();
        match part {
            ContentPart::Text(text) if text.annotations().is_empty() => {
                only_fields(&value, &["type", "text", "annotations"])?;
                if value
                    .get("annotations")
                    .is_some_and(|value| value.as_array().is_none_or(|items| !items.is_empty()))
                {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                value.insert(
                    "type".into(),
                    json!(if chat {
                        "text"
                    } else if current.role() == MessageRole::Assistant {
                        "output_text"
                    } else {
                        "input_text"
                    }),
                );
                value.insert("text".into(), json!(text.text().as_str()));
            }
            ContentPart::Refusal(text) if !chat => {
                only_fields(&value, &["type", "refusal"])?;
                value.insert("type".into(), json!("refusal"));
                value.insert("refusal".into(), json!(text.as_str()));
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
        content.push(Value::Object(value));
    }
    let scalar = !matches!(encoded.get("content"), Some(Value::Array(_)))
        && content.len() == 1
        && content[0].get("text").and_then(Value::as_str).is_some()
        && current
            .content()
            .iter()
            .all(|part| matches!(part, ContentPart::Text(_)));
    encoded.insert(
        "content".into(),
        if scalar {
            content[0]["text"].clone()
        } else if chat && content.is_empty() {
            Value::Null
        } else {
            Value::Array(content)
        },
    );
    encoded.insert(
        "role".into(),
        json!(match current.role() {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }),
    );
    if chat {
        encoded.remove("refusal");
        if let Some(value) = refusal {
            encoded.insert("refusal".into(), value);
        }
    } else if source.is_none() {
        encoded.insert("type".into(), json!("message"));
    }
    Ok(Value::Object(encoded))
}

fn instruction(
    value: &Instruction,
    old: Option<&Instruction>,
    source: Option<&Value>,
    chat: bool,
) -> Result<Value, StaticCodecError> {
    if old == Some(value) {
        return source.cloned().ok_or(StaticCodecError::InvalidShape);
    }
    if !matches!(
        value.origin(),
        InstructionOrigin::Downstream | InstructionOrigin::GatewayPolicy
    ) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let mut result = source
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    only_fields(
        &result,
        if chat {
            &["role", "content"]
        } else {
            &["type", "id", "role", "content", "status"]
        },
    )?;
    // A flattened instruction cannot safely retain individual part metadata after edits.
    if let Some(Value::Array(parts)) = result.get("content") {
        for part in parts {
            only_fields(
                part.as_object().ok_or(StaticCodecError::InvalidShape)?,
                &["type", "text"],
            )?;
        }
    }
    result.insert(
        "role".into(),
        json!(match value.authority() {
            InstructionAuthority::System => "system",
            InstructionAuthority::Developer => "developer",
        }),
    );
    result.insert("content".into(), json!(value.text().as_str()));
    if !chat && source.is_none() {
        result.insert("type".into(), json!("message"));
    }
    Ok(Value::Object(result))
}
