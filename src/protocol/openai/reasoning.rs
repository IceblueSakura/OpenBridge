//! Responses reasoning object and readable reasoning items.
use super::{
    CodecError, Profile,
    common::{fields, object, string},
};
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::task::generation::{
        ItemId, ItemLifecycle, MAX_TEXT_BYTES, PartId, ReasoningContent, ReasoningEffort,
        ReasoningItem, ReasoningPresence, ReasoningRequest, ReasoningSummary,
    },
};
use serde_json::{Map, Value, json};

pub(super) fn request(o: &Map<String, Value>) -> Result<ReasoningRequest, CodecError> {
    let encrypted = match o.get("include") {
        None => false,
        Some(Value::Array(values))
            if values
                .iter()
                .all(|v| v.as_str() == Some("reasoning.encrypted_content")) =>
        {
            !values.is_empty()
        }
        _ => return Err(CodecError::Unsupported("include".into())),
    };
    let Some(value) = o.get("reasoning") else {
        return Ok(ReasoningRequest::absent().with_encrypted_output(encrypted));
    };
    let reasoning = object(value)?;
    fields(reasoning, &["effort", "summary"])?;
    Ok(ReasoningRequest::present(
        optional_label(reasoning, "effort", effort)?,
        match reasoning.get("summary") {
            Some(Value::Bool(false)) => Some(ReasoningSummary::Disabled),
            _ => optional_label(reasoning, "summary", summary)?,
        },
    )
    .with_encrypted_output(encrypted))
}
pub(super) fn chat_request(o: &Map<String, Value>) -> Result<ReasoningRequest, CodecError> {
    let Some(value) = o.get("reasoning_effort") else {
        return Ok(ReasoningRequest::absent());
    };
    let label = value
        .as_str()
        .ok_or(CodecError::Invalid("reasoning_effort"))?;
    Ok(ReasoningRequest::present(Some(effort(label)?), None))
}
pub(super) fn write_request(
    value: &ReasoningRequest,
    o: &mut Map<String, Value>,
    profile: Profile,
) {
    if value.encrypted_output() {
        o.insert("include".into(), json!(["reasoning.encrypted_content"]));
    }
    if value.presence() == ReasoningPresence::Absent {
        return;
    }
    if profile == Profile::Chat {
        if let Some(effort) = value.effort() {
            o.insert(
                "reasoning_effort".into(),
                Value::String(effort_label(effort).into()),
            );
        }
        return;
    }
    let mut reasoning = Map::new();
    if let Some(effort) = value.effort() {
        reasoning.insert("effort".into(), Value::String(effort_label(effort).into()));
    }
    if let Some(summary) = value.summary() {
        reasoning.insert(
            "summary".into(),
            if summary == ReasoningSummary::Disabled {
                Value::Bool(false)
            } else {
                Value::String(summary_label(summary).into())
            },
        );
    }
    o.insert("reasoning".into(), Value::Object(reasoning));
}
pub(super) fn decode_item(
    o: &Map<String, Value>,
    next_part: &mut impl FnMut() -> Result<PartId, CodecError>,
    allow_incomplete: bool,
) -> Result<(ReasoningItem, Option<String>), CodecError> {
    fields(
        o,
        &[
            "id",
            "type",
            "summary",
            "content",
            "encrypted_content",
            "status",
        ],
    )?;
    let status = match o.get("status") {
        None | Some(Value::Null) => ItemLifecycle::Completed,
        Some(Value::String(s)) if s == "completed" => ItemLifecycle::Completed,
        Some(Value::String(s)) if s == "incomplete" && allow_incomplete => {
            ItemLifecycle::Incomplete
        }
        _ => return Err(CodecError::Unsupported("reasoning status".into())),
    };
    let mut parts = Vec::new();
    for part in o
        .get("summary")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("reasoning summary"))?
    {
        let part = object(part)?;
        fields(part, &["type", "text"])?;
        if string(part, "type")? != "summary_text" {
            return Err(CodecError::Unsupported("summary part".into()));
        }
        parts.push((
            next_part()?,
            ReasoningContent::Summary(
                crate::semantic::value::Text::allowing_empty(
                    string(part, "text")?,
                    "summary",
                    MAX_TEXT_BYTES,
                )
                .map_err(|_| CodecError::Limit)?,
            ),
        ));
    }
    if let Some(content) = o.get("content").filter(|value| !value.is_null()) {
        for part in content
            .as_array()
            .ok_or(CodecError::Invalid("reasoning content"))?
        {
            let part = object(part)?;
            fields(part, &["type", "text"])?;
            if string(part, "type")? != "reasoning_text" {
                return Err(CodecError::Unsupported("reasoning text".into()));
            }
            parts.push((
                next_part()?,
                ReasoningContent::Text(
                    crate::semantic::value::Text::allowing_empty(
                        string(part, "text")?,
                        "reasoning text",
                        MAX_TEXT_BYTES,
                    )
                    .map_err(|_| CodecError::Limit)?,
                ),
            ));
        }
    }
    let encrypted = match o.get("encrypted_content") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        _ => return Err(CodecError::Invalid("encrypted reasoning")),
    };
    Ok((ReasoningItem { parts, status }, encrypted))
}
pub(super) fn encode_item(
    id: ItemId,
    item: &ReasoningItem,
    fidelity: &FidelityRecords,
    response: bool,
) -> Value {
    let summary: Vec<_> = item
        .parts
        .iter()
        .filter_map(|(_, part)| match part {
            ReasoningContent::Summary(text) => {
                Some(json!({"type":"summary_text","text":text.as_str()}))
            }
            ReasoningContent::Text(_) => None,
        })
        .collect();
    let content: Vec<_> = item
        .parts
        .iter()
        .filter_map(|(_, part)| match part {
            ReasoningContent::Text(text) => {
                Some(json!({"type":"reasoning_text","text":text.as_str()}))
            }
            ReasoningContent::Summary(_) => None,
        })
        .collect();
    let mut value = json!({"type":"reasoning","summary":summary});
    if !content.is_empty() {
        value["content"] = json!(content);
    }
    if let Some(encrypted) = fidelity.encrypted_reasoning_replay(id) {
        value["encrypted_content"] = json!(encrypted);
    }
    if response {
        value["status"] = json!(match item.status {
            ItemLifecycle::Completed => "completed",
            ItemLifecycle::Incomplete => "incomplete",
        });
    }
    value
}
fn optional_label<T>(
    o: &Map<String, Value>,
    key: &'static str,
    parse: fn(&str) -> Result<T, CodecError>,
) -> Result<Option<T>, CodecError> {
    match o.get(key) {
        None => Ok(None),
        Some(value) => Ok(Some(parse(
            value.as_str().ok_or(CodecError::Invalid(key))?,
        )?)),
    }
}
fn effort(value: &str) -> Result<ReasoningEffort, CodecError> {
    Ok(match value {
        "none" => ReasoningEffort::None,
        "minimal" => ReasoningEffort::Minimal,
        "low" => ReasoningEffort::Low,
        "medium" => ReasoningEffort::Medium,
        "high" => ReasoningEffort::High,
        "xhigh" => ReasoningEffort::XHigh,
        "max" => ReasoningEffort::Max,
        _ => return Err(CodecError::Unsupported("reasoning effort".into())),
    })
}
fn summary(value: &str) -> Result<ReasoningSummary, CodecError> {
    Ok(match value {
        "auto" => ReasoningSummary::Auto,
        "concise" => ReasoningSummary::Concise,
        "detailed" => ReasoningSummary::Detailed,
        _ => return Err(CodecError::Unsupported("reasoning summary".into())),
    })
}
fn effort_label(value: ReasoningEffort) -> &'static str {
    match value {
        ReasoningEffort::None => "none",
        ReasoningEffort::Minimal => "minimal",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}
fn summary_label(value: ReasoningSummary) -> &'static str {
    match value {
        ReasoningSummary::Disabled => unreachable!("disabled is a boolean"),
        ReasoningSummary::Auto => "auto",
        ReasoningSummary::Concise => "concise",
        ReasoningSummary::Detailed => "detailed",
    }
}
