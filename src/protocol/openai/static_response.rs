//! Completed single-candidate response closure. Partial, failed and event output remain explicit gaps.
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
    let usage = match o.get("usage") {
        None | Some(Value::Null) => None,
        Some(v) => {
            object(v)?;
            Some((profile, v.clone()))
        }
    };
    Ok(ResponseMetadata {
        id,
        model,
        created,
        usage,
    })
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
    let completion = match string(c, "finish_reason")? {
        "stop" => Completion::Stop,
        "tool_calls" => Completion::ToolCalls,
        _ => {
            return Err(CodecError::Unsupported(
                "non-completed finish reason".into(),
            ));
        }
    };
    let message = object(c.get("message").ok_or(CodecError::Invalid("message"))?)?;
    let mut b = Items::default();
    chat::decode_message(&mut b, message)?;
    Ok(DecodedResponse {
        semantic: GenerationResponse::new(b.items, completion)?,
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
    if string(o, "status")? != "completed"
        || ["error", "incomplete_details"]
            .iter()
            .any(|key| o.get(*key).is_some_and(|v| !v.is_null()))
    {
        return Err(CodecError::Unsupported("non-completed response".into()));
    }
    let output = o
        .get("output")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("output"))?;
    let mut b = Items::default();
    responses::decode_items(&mut b, output, true)?;
    let completion = if b.items.iter().any(|(_, i)| matches!(i, Item::ToolCall(_))) {
        Completion::ToolCalls
    } else {
        Completion::Stop
    };
    Ok(DecodedResponse {
        semantic: GenerationResponse::new(b.items, completion)?,
        fidelity: b.fidelity,
        metadata: metadata(o, Profile::Responses)?,
    })
}
pub fn encode_chat(target: &ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    if target.profile != Profile::Chat {
        return Err(CodecError::ProfileMismatch);
    }
    let messages = chat::encode_items(target.semantic.items());
    let m = target.metadata;
    Ok(
        json!({"id":m.id,"object":"chat.completion","created":m.created,"model":m.model,
        "choices":[{"index":0,"message":messages[0],"finish_reason":match target.semantic.completion(){Completion::Stop=>"stop",Completion::ToolCalls=>"tool_calls"}}],
        "usage":m.usage.as_ref().map(|(_, v)| v)}),
    )
}
pub fn encode_responses(target: &ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    if target.profile != Profile::Responses {
        return Err(CodecError::ProfileMismatch);
    }
    let m = target.metadata;
    Ok(
        json!({"id":m.id,"object":"response","created_at":m.created,"model":m.model,"status":"completed",
        "output":responses::encode_items(target.semantic.items(), target.fidelity, true),"usage":m.usage.as_ref().map(|(_, v)| v)}),
    )
}
