//! Generation event codecs. Protocol indexes are mappings, never semantic identities.
//! Payloads exclude SSE framing. Rejected streams are poisoned and cannot resume.
mod chat;
mod decode;
mod encode;
use super::{CodecError, Profile, ResponseMetadata, common::*};
use crate::lowering::generation::GenerationRepresentationContract;
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::{task::generation::*, value::ReplayOrigin},
};
pub use decode::EventDecoder;
pub use encode::EventEncoder;
use serde_json::{Map, Value};

fn event_fields(o: &Map<String, Value>, allowed: &[&str]) -> Result<(), CodecError> {
    if let Some(key) = o
        .keys()
        .find(|k| k.as_str() != "sequence_number" && !allowed.contains(&k.as_str()))
    {
        return Err(CodecError::Unsupported(key.clone()));
    }
    Ok(())
}
fn index(o: &Map<String, Value>, key: &'static str) -> Result<usize, CodecError> {
    o.get(key)
        .and_then(Value::as_u64)
        .filter(|n| *n < MAX_ITEMS as u64)
        .map(|n| n as usize)
        .ok_or(CodecError::Invalid(key))
}
fn lifecycle(value: &str) -> Result<ItemLifecycle, CodecError> {
    match value {
        "completed" => Ok(ItemLifecycle::Completed),
        "incomplete" => Ok(ItemLifecycle::Incomplete),
        _ => Err(CodecError::Invalid("item status")),
    }
}
fn replay(
    o: &Map<String, Value>,
    origin: Option<&ReplayOrigin>,
    final_value: bool,
) -> Result<Option<ReasoningReplay>, CodecError> {
    match o.get("encrypted_content") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let value = text(s, "encrypted reasoning", MAX_TEXT_BYTES)?;
            Ok(Some(ReasoningReplay {
                origin: origin.cloned(),
                value: if final_value {
                    EncryptedReasoning::Final(value)
                } else {
                    EncryptedReasoning::Partial(value)
                },
            }))
        }
        _ => Err(CodecError::Invalid("encrypted reasoning")),
    }
}
fn kind_name(kind: PartKind) -> &'static str {
    match kind {
        PartKind::Text => "output_text",
        PartKind::Refusal => "refusal",
        PartKind::Summary => "summary_text",
        PartKind::ReasoningText => "reasoning_text",
        PartKind::Arguments => "arguments",
    }
}
fn part_kind(part: &Map<String, Value>) -> Result<PartKind, CodecError> {
    match string(part, "type")? {
        "output_text" => Ok(PartKind::Text),
        "refusal" => Ok(PartKind::Refusal),
        "summary_text" => Ok(PartKind::Summary),
        "reasoning_text" => Ok(PartKind::ReasoningText),
        _ => Err(CodecError::Unsupported("part kind".into())),
    }
}
fn part_text(part: &Map<String, Value>, kind: PartKind) -> Result<&str, CodecError> {
    fields(
        part,
        if kind == PartKind::Refusal {
            &["type", "refusal"]
        } else {
            &["type", "text", "annotations"]
        },
    )?;
    if part
        .get("annotations")
        .is_some_and(|v| !v.as_array().is_some_and(Vec::is_empty))
    {
        return Err(CodecError::Unsupported("annotations".into()));
    }
    string(
        part,
        if kind == PartKind::Refusal {
            "refusal"
        } else {
            "text"
        },
    )
}
fn part_wire(kind: PartKind, text: &str) -> Value {
    if kind == PartKind::Refusal {
        serde_json::json!({"type":"refusal","refusal":text})
    } else if kind == PartKind::Text {
        serde_json::json!({"type":"output_text","text":text,"annotations":[]})
    } else {
        serde_json::json!({"type":kind_name(kind),"text":text})
    }
}
fn item_wire(
    state: &StreamState,
    id: ItemId,
    fidelity: &FidelityRecords,
) -> Result<Value, CodecError> {
    let item = state.item(id)?.snapshot()?;
    super::responses::encode_items(&[(id, item)], fidelity, true)
        .into_iter()
        .next()
        .ok_or(CodecError::Invalid("item snapshot"))
}
