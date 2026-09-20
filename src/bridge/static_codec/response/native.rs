//! Native content lowering with identity-bound source hints, never positional replay.
//!
//! Unmigrated domains may retain their source spelling only while semantically unchanged.
//! Unknown content-dependent fields fail closed on edits rather than acquiring guessed ownership.

use std::collections::BTreeMap;

use super::*;

#[cfg(test)]
mod tests;

pub(super) fn encode(
    protocol: ApiProtocol,
    response: &WireResponse,
    original: &WireResponse,
    source: &Map<String, Value>,
) -> Result<Map<String, Value>, StaticCodecError> {
    let current = &response.semantic;
    let previous = &original.semantic;
    // Identity, lifecycle, usage and error extensions are separate migration domains.
    if current.id() != previous.id()
        || current.status() != previous.status()
        || current.usage() != previous.usage()
        || current.extensions() != previous.extensions()
        || current.failure() != previous.failure()
        || current.candidates().len() != 1
        || previous.candidates().len() != 1
        || current.candidates()[0].id() != previous.candidates()[0].id()
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    if current == previous {
        return Ok(source.clone());
    }
    // Unknown envelope extensions may summarize the old content. Without a dependency contract,
    // unchanged replay is safe but attaching them to transformed output is not.
    match protocol {
        ApiProtocol::ChatCompletions => only_fields(
            source,
            &[
                "id",
                "object",
                "created",
                "model",
                "choices",
                "usage",
                "system_fingerprint",
                "service_tier",
            ],
        )?,
        ApiProtocol::Responses => only_fields(
            source,
            &[
                "id",
                "object",
                "created_at",
                "completed_at",
                "model",
                "status",
                "output",
                "usage",
                "error",
                "incomplete_details",
                "metadata",
                "instructions",
                "tools",
                "tool_choice",
                "parallel_tool_calls",
                "reasoning",
                "text",
                "temperature",
                "top_p",
                "max_output_tokens",
                "previous_response_id",
                "store",
                "truncation",
                "user",
                "service_tier",
                "background",
                "max_tool_calls",
                "prompt_cache_key",
                "prompt_cache_retention",
                "safety_identifier",
            ],
        )?,
    }
    let mut result = source.clone();
    let candidate = &current.candidates()[0];
    let old = &previous.candidates()[0];
    let has_calls = candidate
        .output()
        .iter()
        .any(|item| matches!(item, OutputItem::ToolCall(_)));
    if current.status() == ResponseStatus::Completed
        && matches!(candidate.finish(), Some(FinishReason::ToolCalls)) != has_calls
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    match protocol {
        ApiProtocol::Responses => {
            if candidate.finish() != old.finish() {
                let expected = if candidate
                    .output()
                    .iter()
                    .any(|item| matches!(item, OutputItem::ToolCall(_)))
                {
                    FinishReason::ToolCalls
                } else {
                    FinishReason::Stop
                };
                if current.status() != ResponseStatus::Completed
                    || candidate.finish() != Some(&expected)
                {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
            }
            result.insert(
                "output".to_owned(),
                encode_responses(candidate, old, source)?,
            );
        }
        ApiProtocol::ChatCompletions => encode_chat(&mut result, candidate, old)?,
    }
    Ok(result)
}

fn identity(item: &OutputItem) -> Result<&ItemId, StaticCodecError> {
    match item {
        OutputItem::Message(value) => Ok(value.id()),
        OutputItem::ToolCall(value) => Ok(value.id()),
        OutputItem::Reasoning(value) => Ok(value.id()),
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn only_fields(value: &Map<String, Value>, fields: &[&str]) -> Result<(), StaticCodecError> {
    if value.keys().any(|key| !fields.contains(&key.as_str())) {
        Err(StaticCodecError::UnsupportedSemantics)
    } else {
        Ok(())
    }
}

fn encode_responses(
    candidate: &Candidate,
    old: &Candidate,
    source: &Map<String, Value>,
) -> Result<Value, StaticCodecError> {
    let source_items = source
        .get("output")
        .and_then(Value::as_array)
        .ok_or(StaticCodecError::InvalidShape)?;
    // An empty reasoning item can currently be absent from IR. Do not silently erase it on edits.
    if source_items.len() != old.output().len() {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let originals = old
        .output()
        .iter()
        .map(|item| Ok((identity(item)?, item)))
        .collect::<Result<BTreeMap<_, _>, StaticCodecError>>()?;
    let wire = source_items
        .iter()
        .map(|item| {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or(StaticCodecError::InvalidShape)?;
            Ok((id, item))
        })
        .collect::<Result<BTreeMap<_, _>, StaticCodecError>>()?;
    let mut used_ids: BTreeSet<String> = wire.keys().map(|id| (*id).to_owned()).collect();
    let mut next_id = 0usize;
    let mut output = Vec::new();
    for item in candidate.output() {
        let id = identity(item)?;
        let previous = originals.get(id).copied();
        let source_item = wire.get(id.as_str()).copied();
        if previous == Some(item) {
            output.push(source_item.ok_or(StaticCodecError::InvalidShape)?.clone());
            continue;
        }
        let mut encoded = match source_item {
            Some(value) => value
                .as_object()
                .cloned()
                .ok_or(StaticCodecError::InvalidShape)?,
            None => Map::new(),
        };
        match (item, previous) {
            (OutputItem::Message(message), Some(OutputItem::Message(before))) => {
                only_fields(&encoded, &["id", "type", "role", "status", "content"])?;
                if message.wire_identity() != before.wire_identity() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                let parts = encoded
                    .get("content")
                    .and_then(Value::as_array)
                    .ok_or(StaticCodecError::InvalidShape)?;
                encoded.insert(
                    "content".to_owned(),
                    encode_parts(message, Some((before, parts)))?,
                );
            }
            (OutputItem::Message(message), None) => {
                if message.wire_identity().is_some() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                encoded.extend(
                    json!({"type":"message", "role":"assistant", "status":"completed"})
                        .as_object()
                        .unwrap()
                        .clone(),
                );
                encoded.insert("content".to_owned(), encode_parts(message, None)?);
            }
            (OutputItem::ToolCall(call), Some(OutputItem::ToolCall(before))) => {
                only_fields(
                    &encoded,
                    &["id", "type", "status", "call_id", "name", "arguments"],
                )?;
                if call.wire_identity() != before.wire_identity() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                encode_response_call(&mut encoded, call, Some(before))?;
            }
            (OutputItem::ToolCall(call), None) => {
                if call.wire_identity().is_some() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                encoded.insert("type".to_owned(), json!("function_call"));
                encoded.insert("status".to_owned(), json!("completed"));
                encode_response_call(&mut encoded, call, None)?;
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
        let wire_id = if previous.is_some() {
            id.as_str().to_owned()
        } else {
            // Native Responses requires an output-item ID. Allocate a profile-local wire ID
            // independently of the internal identity; never borrow another source item's ID.
            loop {
                let candidate = format!("ob_output_{next_id}");
                next_id = next_id
                    .checked_add(1)
                    .ok_or(StaticCodecError::LimitExceeded)?;
                if used_ids.insert(candidate.clone()) {
                    break candidate;
                }
            }
        };
        encoded.insert("id".to_owned(), json!(wire_id));
        output.push(Value::Object(encoded));
    }
    Ok(Value::Array(output))
}

fn encode_response_call(
    encoded: &mut Map<String, Value>,
    call: &ToolCall,
    before: Option<&ToolCall>,
) -> Result<(), StaticCodecError> {
    encoded.insert("call_id".to_owned(), json!(call.call_id().as_str()));
    encoded.insert("name".to_owned(), json!(call.tool().as_str()));
    // Preserve JSON argument spelling only when its parsed value remains equivalent.
    if before.is_none_or(|previous| previous.input() != call.input()) {
        encoded.insert("arguments".to_owned(), json!(function_arguments(call)?));
    }
    Ok(())
}

fn encode_parts(
    message: &ResponseMessage,
    previous: Option<(&ResponseMessage, &[Value])>,
) -> Result<Value, StaticCodecError> {
    let mut originals = BTreeMap::new();
    if let Some((before, wire)) = previous {
        // Empty output_text parts are not represented yet. Refuse edits rather than lose them.
        if wire.len() != before.content().len() {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        for ((id, content), value) in before.part_ids().iter().zip(before.content()).zip(wire) {
            originals.insert(*id, (content, value));
        }
    }
    let mut result = Vec::new();
    for (id, part) in message.part_ids().iter().zip(message.content()) {
        let previous = originals.get(id).copied();
        if let Some((old, wire)) = previous {
            if old == part {
                result.push(wire.clone());
                continue;
            }
            let fields = wire.as_object().ok_or(StaticCodecError::InvalidShape)?;
            only_fields(fields, &["type", "text", "refusal", "annotations"])?;
            if fields
                .get("annotations")
                .is_some_and(|value| value.as_array().is_none_or(|array| !array.is_empty()))
            {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
        }
        let value = match part {
            ContentPart::Text(text) if text.annotations().is_empty() => {
                let mut encoded = json!({"type":"output_text", "text":text.text().as_str()});
                if previous.is_none_or(|(_, wire)| wire.get("annotations").is_some()) {
                    encoded["annotations"] = json!([]);
                }
                encoded
            }
            ContentPart::Refusal(text) => json!({"type":"refusal", "refusal":text.as_str()}),
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        };
        result.push(value);
    }
    Ok(Value::Array(result))
}

fn encode_chat(
    result: &mut Map<String, Value>,
    candidate: &Candidate,
    old: &Candidate,
) -> Result<(), StaticCodecError> {
    let choice = result
        .get_mut("choices")
        .and_then(Value::as_array_mut)
        .and_then(|values| values.first_mut())
        .and_then(Value::as_object_mut)
        .ok_or(StaticCodecError::InvalidShape)?;
    // Logprobs and unknown choice extensions may depend on the old generated content.
    only_fields(choice, &["index", "message", "finish_reason", "logprobs"])?;
    if choice.get("logprobs").is_some_and(|value| !value.is_null()) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let message = choice
        .get_mut("message")
        .and_then(Value::as_object_mut)
        .ok_or(StaticCodecError::InvalidShape)?;
    only_fields(
        message,
        &[
            "role",
            "content",
            "refusal",
            "reasoning_content",
            "audio",
            "tool_calls",
        ],
    )?;
    let old_messages: Vec<_> = old
        .output()
        .iter()
        .filter_map(|item| match item {
            OutputItem::Message(value) => Some(value),
            _ => None,
        })
        .collect();
    let new_messages: Vec<_> = candidate
        .output()
        .iter()
        .filter_map(|item| match item {
            OutputItem::Message(value) => Some(value),
            _ => None,
        })
        .collect();
    if new_messages.len() > 1
        || (new_messages.is_empty()
            && !candidate
                .output()
                .iter()
                .any(|item| matches!(item, OutputItem::ToolCall(_))))
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let old_reasoning: Vec<_> = old
        .output()
        .iter()
        .filter(|item| matches!(item, OutputItem::Reasoning(_)))
        .collect();
    let new_reasoning: Vec<_> = candidate
        .output()
        .iter()
        .filter(|item| matches!(item, OutputItem::Reasoning(_)))
        .collect();
    if old_reasoning != new_reasoning {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    // One Chat message can only represent this ordering, not arbitrary interleaved items.
    let mut phase = 0;
    for item in candidate.output() {
        let next = match item {
            OutputItem::Reasoning(_) => 0,
            OutputItem::Message(_) => 1,
            OutputItem::ToolCall(_) => 2,
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        };
        if next < phase {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        phase = next;
    }
    if old_messages != new_messages {
        encode_chat_message(
            message,
            new_messages.first().copied(),
            old_messages.first().copied(),
        )?;
    }
    let previous_calls = old
        .output()
        .iter()
        .filter_map(|item| match item {
            OutputItem::ToolCall(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let source_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let originals = previous_calls
        .iter()
        .zip(source_calls)
        .map(|(call, wire)| (call.id(), (*call, wire)))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Vec::new();
    for item in candidate.output() {
        let OutputItem::ToolCall(call) = item else {
            continue;
        };
        let mut value = if let Some((before, wire)) = originals.get(call.id()) {
            if *before == call {
                calls.push(wire.clone());
                continue;
            }
            let fields = wire.as_object().ok_or(StaticCodecError::InvalidShape)?;
            only_fields(fields, &["id", "type", "function"])?;
            only_fields(
                fields
                    .get("function")
                    .and_then(Value::as_object)
                    .ok_or(StaticCodecError::InvalidShape)?,
                &["name", "arguments"],
            )?;
            if call.wire_identity() != before.wire_identity() {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
            wire.clone()
        } else {
            if call.wire_identity().is_some() {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
            json!({"type":"function","function":{}})
        };
        value["id"] = json!(call.call_id().as_str());
        value["function"]["name"] = json!(call.tool().as_str());
        if originals
            .get(call.id())
            .is_none_or(|(old, _)| old.input() != call.input())
        {
            value["function"]["arguments"] = json!(function_arguments(call)?);
        }
        calls.push(value);
    }
    if calls.is_empty() {
        if !previous_calls.is_empty() {
            message.remove("tool_calls");
        }
    } else {
        message.insert("tool_calls".to_owned(), Value::Array(calls));
    }
    choice.insert(
        "finish_reason".to_owned(),
        json!(encode_chat_finish(candidate.finish())?),
    );
    Ok(())
}

fn encode_chat_message(
    target: &mut Map<String, Value>,
    message: Option<&ResponseMessage>,
    previous: Option<&ResponseMessage>,
) -> Result<(), StaticCodecError> {
    let mut text = None;
    let mut refusal = None;
    let mut audio = false;
    if let Some(message) = message {
        if message.wire_identity() != previous.and_then(ResponseMessage::wire_identity) {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        for (id, part) in message.part_ids().iter().zip(message.content()) {
            match part {
                ContentPart::Text(value)
                    if value.annotations().is_empty()
                        && text.is_none()
                        && refusal.is_none()
                        && !audio =>
                {
                    text = Some(value.text().as_str());
                }
                ContentPart::Refusal(value) if text.is_none() && refusal.is_none() && !audio => {
                    refusal = Some(value.as_str())
                }
                ContentPart::Resource(Resource::Audio(_)) if !audio => {
                    let old = previous
                        .filter(|old| old.id() == message.id())
                        .and_then(|old| {
                            old.part_ids()
                                .iter()
                                .position(|candidate| candidate == id)
                                .map(|index| &old.content()[index])
                        });
                    if old != Some(part) || !target.contains_key("audio") {
                        return Err(StaticCodecError::UnsupportedSemantics);
                    }
                    audio = true;
                }
                _ => return Err(StaticCodecError::UnsupportedSemantics),
            }
        }
    }
    target.insert(
        "content".to_owned(),
        text.map_or(Value::Null, |value| json!(value)),
    );
    match refusal {
        Some(value) => {
            target.insert("refusal".to_owned(), json!(value));
        }
        None => {
            target.remove("refusal");
        }
    }
    if !audio {
        target.remove("audio");
    }
    Ok(())
}
