//! Closed OpenBridge-client attachment schema. Never a credential/scope locator.
use super::{CodecError, common::*};
use crate::{protocol::fidelity::FidelityRecords, semantic::task::generation::*};
use serde_json::{Map, Value, json};
mod usage;
pub(crate) use usage::{read as read_usage, standard as base_usage, write as write_usage};
pub(crate) const FIELD: &str = "_openbridge";
pub(crate) fn enabled(adaptation: &crate::protocol::adaptation::Adaptation) -> bool {
    adaptation.client == crate::protocol::adaptation::ClientExtension::GenerationV1
}
fn extension<'a>(value: &'a Value, allowed: &[&str]) -> Result<&'a Map<String, Value>, CodecError> {
    let object = object(value)?;
    fields(object, allowed)?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(CodecError::Invalid("client extension version"));
    }
    Ok(object)
}
pub(super) fn clean_item(
    value: &Value,
    enabled: bool,
) -> Result<(Value, Option<Value>), CodecError> {
    let mut clean = object(value)?.clone();
    let extra = clean.remove(FIELD);
    if extra.is_some() && !enabled {
        return Err(CodecError::Unsupported("client extension".into()));
    }
    Ok((Value::Object(clean), extra))
}
fn owner(
    name: &str,
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
) -> Result<ItemId, CodecError> {
    items
        .iter()
        .find_map(|(id, _)| (fidelity.response_item_id(*id) == Some(name)).then_some(*id))
        .ok_or(CodecError::Invalid("client owner reference"))
}
pub(super) fn apply_item(
    item: &mut Item,
    extra: Option<&Value>,
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
) -> Result<(), CodecError> {
    let Some(extra) = extra else { return Ok(()) };
    let extra = extension(
        extra,
        &[
            "version",
            "arguments",
            "message",
            "replay",
            "output",
            "execution",
        ],
    )?;
    let mut used = vec!["version"];
    match item {
        Item::ToolCall(call) => {
            if let Some(kind) = extra.get("arguments") {
                used.push("arguments");
                let raw = call
                    .arguments
                    .as_raw()
                    .ok_or(CodecError::Invalid("argument authority"))?;
                call.arguments = match kind.as_str() {
                    Some("json") => {
                        ToolArguments::Structured(StructuredValue::from_bytes(raw.as_bytes())?)
                    }
                    Some("json_partial") => ToolArguments::StructuredPartial(raw.to_owned()),
                    _ => return Err(CodecError::Invalid("argument authority")),
                };
            }
            if let Some(reference) = extra.get("message") {
                used.push("message");
                let reference = owner(
                    reference
                        .as_str()
                        .ok_or(CodecError::Invalid("message reference"))?,
                    items,
                    fidelity,
                )?;
                if !items.iter().any(|(id, item)| {
                    *id == reference
                        && matches!(item,Item::Message(m) if m.role==MessageRole::Assistant)
                }) {
                    return Err(CodecError::Invalid("message reference owner"));
                }
                call.message = Some(reference);
            }
        }
        Item::Reasoning(reasoning) => {
            if let Some(value) = extra.get("replay") {
                used.push("replay");
                if reasoning.replay.is_some() {
                    return Err(CodecError::Invalid("competing replay authority"));
                }
                let value = object(value)?;
                fields(value, &["format", "phase", "value"])?;
                let format = match string(value, "format")? {
                    "google-interactions-v1-thought" => ReplayFormat::GoogleInteractionsV1Thought,
                    _ => return Err(CodecError::Invalid("replay format")),
                };
                let token = text(string(value, "value")?, "opaque replay", MAX_TEXT_BYTES)?;
                reasoning.replay = Some(match string(value, "phase")? {
                    "partial" => ReplayValue::partial(format, token),
                    "final" => ReplayValue::final_value(format, token),
                    _ => return Err(CodecError::Invalid("replay phase")),
                });
            }
        }
        Item::ToolResult(result) | Item::CustomResult(result) => {
            if let Some(kind) = extra.get("output") {
                used.push("output");
                if kind.as_str() != Some("json") {
                    return Err(CodecError::Invalid("output authority"));
                }
                let ToolOutput::Text(raw) = &result.output else {
                    return Err(CodecError::Invalid("output authority"));
                };
                result.output =
                    ToolOutput::Structured(StructuredValue::from_bytes(raw.as_bytes())?);
            }
            if let Some(value) = extra.get("execution") {
                used.push("execution");
                let value = object(value)?;
                fields(value, &["status", "code"])?;
                result.execution = Some(match string(value, "status")? {
                    "succeeded" if !value.contains_key("code") => ToolExecution::Succeeded,
                    "failed" => ToolExecution::Failed {
                        code: value
                            .get("code")
                            .map(|value| {
                                text(
                                    value
                                        .as_str()
                                        .ok_or(CodecError::Invalid("execution code"))?,
                                    "execution code",
                                    128,
                                )
                            })
                            .transpose()?,
                    },
                    _ => return Err(CodecError::Invalid("execution report")),
                });
            }
        }
        _ => {}
    }
    fields(extra, &used)?;
    if used.len() == 1 {
        return Err(CodecError::Invalid("empty client attachment"));
    }
    Ok(())
}
pub(super) fn wire_id(id: ItemId, fidelity: &FidelityRecords) -> String {
    fidelity
        .response_item_id(id)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("item_{}", id.get()))
}
pub(super) fn argument_text(arguments: &ToolArguments) -> String {
    match arguments {
        ToolArguments::Raw(value) | ToolArguments::StructuredPartial(value) => value.clone(),
        ToolArguments::Structured(value) => value.value().to_string(),
    }
}
pub(super) fn write_item(
    value: &mut Value,
    id: ItemId,
    item: &Item,
    fidelity: &FidelityRecords,
) -> Result<(), CodecError> {
    let mut extra = Map::new();
    extra.insert("version".into(), json!(1));
    match item {
        Item::ToolCall(call) => {
            match &call.arguments {
                ToolArguments::Structured(_) => {
                    extra.insert("arguments".into(), json!("json"));
                }
                ToolArguments::StructuredPartial(_) => {
                    extra.insert("arguments".into(), json!("json_partial"));
                }
                _ => {}
            }
            if let Some(reference) = call.message {
                extra.insert("message".into(), json!(wire_id(reference, fidelity)));
            }
        }
        Item::Reasoning(reasoning) => {
            if let Some(replay) = &reasoning.replay
                && replay.format() == ReplayFormat::GoogleInteractionsV1Thought
            {
                value
                    .as_object_mut()
                    .ok_or(CodecError::Invalid("item"))?
                    .remove("encrypted_content");
                extra.insert("replay".into(),json!({"format":"google-interactions-v1-thought","phase":if replay.replay_token().is_some(){"final"}else{"partial"},"value":replay.as_str()}));
            }
        }
        Item::ToolResult(result) | Item::CustomResult(result) => {
            if matches!(result.output, ToolOutput::Structured(_)) {
                extra.insert("output".into(), json!("json"));
            }
            if let Some(execution) = &result.execution {
                extra.insert(
                    "execution".into(),
                    match execution {
                        ToolExecution::Succeeded => json!({"status":"succeeded"}),
                        ToolExecution::Failed { code } => {
                            let mut value = json!({"status":"failed"});
                            if let Some(code) = code {
                                value["code"] = json!(code.as_str());
                            }
                            value
                        }
                    },
                );
            }
        }
        _ => {}
    }
    // Attachment references always point at a delivered identity, not local ItemId.
    if extra.len() > 1 {
        value[FIELD] = Value::Object(extra);
        value["id"] = json!(wire_id(id, fidelity));
    }
    Ok(())
}
pub(super) fn read_progress(
    value: Option<&Value>,
    enabled: bool,
) -> Result<InteractionProgress, CodecError> {
    let Some(value) = value else {
        return Ok(InteractionProgress::Unreported);
    };
    if !enabled {
        return Err(CodecError::Unsupported("client extension".into()));
    }
    let value = extension(value, &["version", "progress", "usage"])?;
    if !value.contains_key("progress") {
        if !value.contains_key("usage") {
            return Err(CodecError::Invalid("empty root attachment"));
        }
        return Ok(InteractionProgress::Unreported);
    }
    Ok(match string(value, "progress")? {
        "turn_finished" => InteractionProgress::TurnFinished,
        "awaiting_tool_results" => InteractionProgress::AwaitingToolResults,
        "needs_continuation" => InteractionProgress::NeedsContinuation,
        _ => return Err(CodecError::Invalid("interaction progress")),
    })
}
pub(super) fn write_progress(value: &mut Value, progress: InteractionProgress) {
    if progress == InteractionProgress::Unreported {
        return;
    }
    let name = match progress {
        InteractionProgress::TurnFinished => "turn_finished",
        InteractionProgress::AwaitingToolResults => "awaiting_tool_results",
        InteractionProgress::NeedsContinuation => "needs_continuation",
        InteractionProgress::Unreported => unreachable!(),
    };
    value[FIELD] = json!({"version":1,"progress":name});
}
