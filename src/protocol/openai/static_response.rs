//! Single-candidate text and function response closure, including non-success terminals.
use super::{
    CodecError, DecodedResponse, Profile, ResponseMetadata, ResponseRepresentation, chat,
    common::*, responses,
};
use crate::semantic::task::generation::*;
use serde_json::{Map, Value, json};

fn metadata(o: &Map<String, Value>, profile: Profile) -> Result<ResponseMetadata, CodecError> {
    let created_key = if profile == Profile::Chat {
        "created"
    } else {
        "created_at"
    };
    let id = text(string(o, "id")?, "response id", 256)?
        .as_str()
        .to_owned();
    let model = text(string(o, "model")?, "response model", 256)?
        .as_str()
        .to_owned();
    let created = o
        .get(created_key)
        .and_then(Value::as_u64)
        .ok_or(CodecError::Invalid("created time"))?;
    Ok(ResponseMetadata { id, model, created })
}
fn usage(value: Option<&Value>, profile: Profile) -> Result<Option<Usage>, CodecError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let usage = object(value)?;
    let (input_key, output_key, input_details, output_details) = match profile {
        Profile::Chat => (
            "prompt_tokens",
            "completion_tokens",
            "prompt_tokens_details",
            "completion_tokens_details",
        ),
        Profile::Responses => (
            "input_tokens",
            "output_tokens",
            "input_tokens_details",
            "output_tokens_details",
        ),
    };
    fields(
        usage,
        &[
            input_key,
            output_key,
            "total_tokens",
            input_details,
            output_details,
        ],
    )?;
    let parsed = Usage {
        input_tokens: count(usage, input_key)?,
        output_tokens: count(usage, output_key)?,
        total_tokens: count(usage, "total_tokens")?,
        cached_input_tokens: detail(usage, input_details, "cached_tokens")?,
        reasoning_tokens: detail(usage, output_details, "reasoning_tokens")?,
    };
    if parsed.input_tokens.checked_add(parsed.output_tokens) != Some(parsed.total_tokens) {
        return Err(CodecError::Invalid("usage total"));
    }
    Ok(Some(parsed))
}
fn count(o: &Map<String, Value>, key: &'static str) -> Result<u64, CodecError> {
    o.get(key)
        .and_then(Value::as_u64)
        .ok_or(CodecError::Invalid(key))
}
fn detail(
    o: &Map<String, Value>,
    key: &'static str,
    field: &'static str,
) -> Result<Option<u64>, CodecError> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let details = object(value)?;
            fields(details, &[field])?;
            match details.get(field) {
                None | Some(Value::Null) => Ok(None),
                Some(value) => Ok(Some(
                    value.as_u64().ok_or(CodecError::Invalid("usage detail"))?,
                )),
            }
        }
    }
}
fn encode_usage(usage: Usage, profile: Profile) -> Value {
    let mut value = match profile {
        Profile::Chat => json!({
            "prompt_tokens": usage.input_tokens,
            "completion_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens
        }),
        Profile::Responses => json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens
        }),
    };
    let object = value.as_object_mut().expect("usage object");
    if let Some(cached) = usage.cached_input_tokens {
        let key = if profile == Profile::Chat {
            "prompt_tokens_details"
        } else {
            "input_tokens_details"
        };
        object.insert(key.into(), json!({"cached_tokens": cached}));
    }
    if let Some(reasoning) = usage.reasoning_tokens {
        let key = if profile == Profile::Chat {
            "completion_tokens_details"
        } else {
            "output_tokens_details"
        };
        object.insert(key.into(), json!({"reasoning_tokens": reasoning}));
    }
    value
}
pub fn decode_chat(v: &Value) -> Result<DecodedResponse, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    fields(o, &["id", "object", "created", "model", "choices", "usage"])?;
    if string(o, "object")? != "chat.completion" {
        return Err(CodecError::Invalid("response object"));
    }
    let choices = o
        .get("choices")
        .and_then(Value::as_array)
        .filter(|a| a.len() == 1)
        .ok_or(CodecError::Unsupported("candidate count".into()))?;
    let c = object(&choices[0])?;
    fields(c, &["index", "message", "finish_reason", "logprobs"])?;
    if c.get("index").and_then(Value::as_u64) != Some(0) {
        return Err(CodecError::Unsupported("candidate index".into()));
    }
    if c.get("logprobs").is_some_and(|v| !v.is_null()) {
        return Err(CodecError::Unsupported("logprobs".into()));
    }
    let outcome = match string(c, "finish_reason")? {
        "stop" => Outcome::Completed(Completion::Stop),
        "tool_calls" => Outcome::Completed(Completion::ToolCalls),
        "length" => Outcome::Incomplete,
        _ => {
            return Err(CodecError::Unsupported("finish reason".into()));
        }
    };
    let message = object(c.get("message").ok_or(CodecError::Invalid("message"))?)?;
    let mut b = Items::default();
    chat::decode_message(&mut b, message)?;
    Ok(DecodedResponse {
        semantic: response_with_usage(b.items, outcome, usage(o.get("usage"), Profile::Chat)?)?,
        fidelity: b.fidelity,
        metadata: metadata(o, Profile::Chat)?,
    })
}
pub fn decode_responses(v: &Value) -> Result<DecodedResponse, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    fields(
        o,
        &[
            "id",
            "object",
            "created_at",
            "model",
            "status",
            "output",
            "usage",
            "error",
            "incomplete_details",
        ],
    )?;
    if string(o, "object")? != "response" {
        return Err(CodecError::Invalid("response object"));
    }
    if ["error", "incomplete_details"]
        .iter()
        .any(|key| o.get(*key).is_some_and(|v| !v.is_null()))
    {
        return Err(CodecError::Unsupported("terminal details".into()));
    }
    let outcome = match string(o, "status")? {
        "completed" => None,
        "incomplete" => Some(Outcome::Incomplete),
        "failed" => Some(Outcome::Failed),
        _ => return Err(CodecError::Unsupported("response status".into())),
    };
    let output = o
        .get("output")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("output"))?;
    let mut b = Items::default();
    let item_status = if outcome.is_none() {
        "completed"
    } else {
        "incomplete"
    };
    responses::decode_items(&mut b, output, true, item_status)?;
    let outcome = outcome.unwrap_or(
        if b.items.iter().any(|(_, i)| matches!(i, Item::ToolCall(_))) {
            Outcome::Completed(Completion::ToolCalls)
        } else {
            Outcome::Completed(Completion::Stop)
        },
    );
    Ok(DecodedResponse {
        semantic: response_with_usage(
            b.items,
            outcome,
            usage(o.get("usage"), Profile::Responses)?,
        )?,
        fidelity: b.fidelity,
        metadata: metadata(o, Profile::Responses)?,
    })
}
fn response_with_usage(
    items: Vec<(ItemId, Item)>,
    outcome: Outcome,
    usage: Option<Usage>,
) -> Result<GenerationResponse, CodecError> {
    let response = match outcome {
        Outcome::Completed(completion) => GenerationResponse::new(items, completion)?,
        outcome => GenerationResponse::unfinished(items, outcome)?,
    };
    Ok(match usage {
        Some(usage) => response.with_usage(usage)?,
        None => response,
    })
}
pub fn encode_chat(target: &ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    if target.profile != Profile::Chat {
        return Err(CodecError::ProfileMismatch);
    }
    let finish = match target.semantic.outcome() {
        Outcome::Completed(Completion::Stop) => "stop",
        Outcome::Completed(Completion::ToolCalls) => "tool_calls",
        Outcome::Incomplete => "length",
        Outcome::Failed => return Err(CodecError::Unsupported("failed terminal".into())),
    };
    let messages = chat::encode_items(target.semantic.items());
    let message = messages
        .first()
        .ok_or(CodecError::Invalid("empty candidate"))?;
    let m = target.metadata;
    Ok(
        json!({"id":m.id,"object":"chat.completion","created":m.created,"model":m.model,
        "choices":[{"index":0,"message":message,"finish_reason":finish}],
        "usage":target.semantic.usage().map(|usage| encode_usage(usage, Profile::Chat))}),
    )
}
pub fn encode_responses(target: &ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    if target.profile != Profile::Responses {
        return Err(CodecError::ProfileMismatch);
    }
    let (status, item_status) = match target.semantic.outcome() {
        Outcome::Completed(_) => ("completed", "completed"),
        Outcome::Incomplete => ("incomplete", "incomplete"),
        Outcome::Failed => ("failed", "incomplete"),
    };
    let mut output = responses::encode_items(target.semantic.items(), target.fidelity, true);
    for item in &mut output {
        if item.get("status").is_some() {
            item["status"] = json!(item_status);
        }
    }
    let m = target.metadata;
    Ok(
        json!({"id":m.id,"object":"response","created_at":m.created,"model":m.model,"status":status,
        "output":output,"usage":target.semantic.usage().map(|usage| encode_usage(usage, Profile::Responses))}),
    )
}
