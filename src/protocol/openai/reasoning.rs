//! Responses reasoning object and readable reasoning items.
use super::{
    CodecError, Profile,
    common::{fields, object, put_presence, read_presence, string},
};
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::task::generation::{
        ItemId, ItemLifecycle, MAX_TEXT_BYTES, PartId, ReasoningContent, ReasoningContext,
        ReasoningEffort, ReasoningItem, ReasoningMode, ReasoningPresence, ReasoningRequest,
        ReasoningSummary,
    },
};
use serde_json::{Map, Value, json};

pub(super) fn request(o: &Map<String, Value>) -> Result<ReasoningRequest, CodecError> {
    let encrypted = match o.get("include") {
        None | Some(Value::Null) => false,
        Some(Value::Array(values))
            if values.iter().all(|v| {
                matches!(
                    v.as_str(),
                    Some("reasoning.encrypted_content" | "message.output_text.logprobs")
                )
            }) =>
        {
            values.iter().any(|v| v == "reasoning.encrypted_content")
        }
        _ => return Err(CodecError::Unsupported("include".into())),
    };
    let Some(value) = o.get("reasoning") else {
        return Ok(ReasoningRequest::absent().with_encrypted_output(encrypted));
    };
    if value.is_null() {
        let mut r = ReasoningRequest::absent().with_encrypted_output(encrypted);
        r.presence = ReasoningPresence::Null;
        return Ok(r);
    }
    let reasoning = object(value)?;
    fields(
        reasoning,
        &["effort", "summary", "generate_summary", "context", "mode"],
    )?;
    let mut r = ReasoningRequest::present(None, None).with_encrypted_output(encrypted);
    r.effort = optional_label(reasoning, "effort", effort)?;
    r.summary = if reasoning.get("summary") == Some(&Value::Bool(false)) {
        crate::semantic::value::Presence::Value(ReasoningSummary::Disabled)
    } else {
        optional_label(reasoning, "summary", summary)?
    };
    if reasoning.contains_key("generate_summary") {
        let old = optional_label(reasoning, "generate_summary", summary)?;
        if !r.summary.is_absent() && r.summary != old {
            return Err(CodecError::Invalid("conflicting summary controls"));
        }
        r.summary = old;
    }
    r.context = optional_label(reasoning, "context", |s| {
        Ok(match s {
            "auto" => ReasoningContext::Auto,
            "current_turn" => ReasoningContext::CurrentTurn,
            "all_turns" => ReasoningContext::AllTurns,
            _ => return Err(CodecError::Unsupported("reasoning context".into())),
        })
    })?;
    r.mode = optional_label(reasoning, "mode", |s| {
        Ok(match s {
            "standard" => ReasoningMode::Standard,
            "pro" => ReasoningMode::Pro,
            _ => return Err(CodecError::Unsupported("reasoning mode".into())),
        })
    })?;
    r.validate()?;
    Ok(r)
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
    if value.presence() == ReasoningPresence::Null {
        o.insert("reasoning".into(), Value::Null);
        return;
    }
    let mut reasoning = Map::new();
    put_presence(&mut reasoning, "effort", &value.effort, |v| {
        json!(effort_label(*v))
    });
    put_presence(&mut reasoning, "summary", &value.summary, |v| {
        if *v == ReasoningSummary::Disabled {
            json!(false)
        } else {
            json!(summary_label(*v))
        }
    });
    put_presence(&mut reasoning, "context", &value.context, |v| {
        json!(match v {
            ReasoningContext::Auto => "auto",
            ReasoningContext::CurrentTurn => "current_turn",
            ReasoningContext::AllTurns => "all_turns",
        })
    });
    put_presence(&mut reasoning, "mode", &value.mode, |v| {
        json!(match v {
            ReasoningMode::Standard => "standard",
            ReasoningMode::Pro => "pro",
        })
    });
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
        Some(Value::String(s)) if s == "in_progress" && allow_incomplete => {
            ItemLifecycle::InProgress
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
    let encrypted = if response {
        fidelity.replay(id).map(|r| r.value.as_str())
    } else {
        fidelity.encrypted_reasoning_replay(id)
    };
    if let Some(encrypted) = encrypted {
        value["encrypted_content"] = json!(encrypted);
    }
    if response {
        value["status"] = json!(match item.status {
            ItemLifecycle::Completed => "completed",
            ItemLifecycle::Incomplete => "incomplete",
            ItemLifecycle::InProgress => "in_progress",
        });
    }
    value
}
fn optional_label<T>(
    o: &Map<String, Value>,
    key: &'static str,
    parse: fn(&str) -> Result<T, CodecError>,
) -> Result<crate::semantic::value::Presence<T>, CodecError> {
    read_presence(o, key, |v| {
        parse(v.as_str().ok_or(CodecError::Invalid(key))?)
    })
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
