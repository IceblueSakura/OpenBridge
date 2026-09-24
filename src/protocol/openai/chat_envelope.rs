//! Complete single-candidate Chat envelopes; task mapping stays in `chat`.
use super::{
    CodecError, DecodedRequest, DecodedResponse, RequestRepresentation, ResponseRepresentation,
    chat, common::*,
};
use crate::semantic::value::Presence;
use serde_json::{Map, Value, json};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StreamOptions {
    pub include_usage: Presence<bool>,
    pub include_obfuscation: Presence<bool>,
}
impl StreamOptions {
    pub fn usage(&self) -> bool {
        self.include_usage == Presence::Value(true)
    }
    pub fn obfuscation(&self) -> bool {
        self.include_obfuscation != Presence::Value(false)
    }
    pub(super) fn validate(&self) -> Result<(), CodecError> {
        if self.include_usage == Presence::Null || self.include_obfuscation == Presence::Null {
            return Err(CodecError::Invalid("Chat stream options"));
        }
        Ok(())
    }
    fn read(v: &Value) -> Result<Self, CodecError> {
        let o = object(v)?;
        fields(o, &["include_usage", "include_obfuscation"])?;
        let boolean = |v: &Value| v.as_bool().ok_or(CodecError::Invalid("Chat stream option"));
        let s = Self {
            include_usage: read_presence(o, "include_usage", boolean)?,
            include_obfuscation: read_presence(o, "include_obfuscation", boolean)?,
        };
        s.validate()?;
        Ok(s)
    }
    fn write(&self) -> Value {
        let mut o = Map::new();
        put_presence(&mut o, "include_usage", &self.include_usage, |v| json!(v));
        put_presence(
            &mut o,
            "include_obfuscation",
            &self.include_obfuscation,
            |v| json!(v),
        );
        Value::Object(o)
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestContext {
    pub model: String,
    pub n: Presence<u64>,
    pub stream: Presence<bool>,
    pub stream_options: Presence<StreamOptions>,
}
impl RequestContext {
    pub fn streaming(&self) -> bool {
        self.stream == Presence::Value(true)
    }
    pub fn validate(&self) -> Result<(), CodecError> {
        text(&self.model, "model", 256)?;
        if self.n.value().is_some_and(|n| *n != 1) {
            return Err(CodecError::Unsupported("candidate count".into()));
        }
        if let Some(options) = self.stream_options.value() {
            if !self.streaming() {
                return Err(CodecError::Invalid("stream_options without streaming"));
            }
            options.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedChatRequest {
    pub task: DecodedRequest,
    pub context: RequestContext,
}
/// Strict raw JSON admission; body collection must be bounded separately by the caller.
pub fn decode_request_bytes(bytes: &[u8]) -> Result<DecodedChatRequest, CodecError> {
    decode_request(&super::json::decode(bytes)?)
}
/// Pre-parsed values cannot prove original raw-byte or duplicate-key validity.
pub fn decode_request(v: &Value) -> Result<DecodedChatRequest, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    let allowed: Vec<_> = chat::FIELDS
        .iter()
        .copied()
        .chain(["model", "n", "stream", "stream_options"])
        .collect();
    fields(o, &allowed)?;
    let context = RequestContext {
        model: string(o, "model")?.into(),
        n: read_presence(o, "n", |v| v.as_u64().ok_or(CodecError::Invalid("n")))?,
        stream: read_presence(o, "stream", |v| {
            v.as_bool().ok_or(CodecError::Invalid("stream"))
        })?,
        stream_options: read_presence(o, "stream_options", StreamOptions::read)?,
    };
    context.validate()?;
    let task: Map<_, _> = o
        .iter()
        .filter(|(k, _)| chat::FIELDS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(DecodedChatRequest {
        task: chat::decode_generation(&Value::Object(task))?,
        context,
    })
}
pub fn encode_request(
    target: &RequestRepresentation<'_>,
    context: &RequestContext,
) -> Result<Value, CodecError> {
    context.validate()?;
    let mut v = chat::encode_generation(target)?;
    let o = v.as_object_mut().expect("object");
    o.insert("model".into(), json!(context.model));
    put_presence(o, "n", &context.n, |v| json!(v));
    put_presence(o, "stream", &context.stream, |v| json!(v));
    put_presence(
        o,
        "stream_options",
        &context.stream_options,
        StreamOptions::write,
    );
    bounded(&v)?;
    Ok(v)
}
pub fn decode_response_bytes(bytes: &[u8]) -> Result<DecodedResponse, CodecError> {
    decode_response(&super::json::decode(bytes)?)
}
/// Validate a complete single-candidate response, not a permissive message-history abbreviation.
pub fn decode_response(v: &Value) -> Result<DecodedResponse, CodecError> {
    bounded(v)?;
    validate_response(v)?;
    chat::decode_response(v)
}
pub fn encode_response(target: &ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    let v = chat::encode_response(target)?;
    validate_response(&v)?;
    Ok(v)
}
fn validate_response(v: &Value) -> Result<(), CodecError> {
    headers(v, "chat.completion")?;
    let choices = v
        .get("choices")
        .and_then(Value::as_array)
        .filter(|a| a.len() == 1)
        .ok_or(CodecError::Unsupported("candidate count".into()))?;
    if choices[0].pointer("/message/role").and_then(Value::as_str) != Some("assistant") {
        return Err(CodecError::Invalid("Chat output role"));
    }
    Ok(())
}
pub(super) fn headers(v: &Value, kind: &str) -> Result<(), CodecError> {
    let o = object(v)?;
    fields(o, &["id", "object", "created", "model", "choices", "usage"])?;
    text(string(o, "id")?, "response id", 256)?;
    text(string(o, "model")?, "response model", 256)?;
    if string(o, "object")? != kind || o.get("created").and_then(Value::as_u64).is_none() {
        return Err(CodecError::Invalid("Chat response headers"));
    }
    Ok(())
}
