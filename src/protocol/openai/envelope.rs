//! Complete stateless Responses envelopes. Execution hints never become task instructions.
use super::{
    CodecError, DecodedRequest, Profile, RequestRepresentation, ResponseMetadata, common::*,
    settings,
};
use crate::semantic::{
    task::generation::*,
    value::{Presence, json_size},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTier {
    Auto,
    Default,
    Flex,
    Fast,
    Priority,
    Scale,
    Ultrafast,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheRetention {
    InMemory,
    #[serde(rename = "24h")]
    Day,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheOptions {
    pub mode: CacheMode,
    pub ttl: CacheTtl,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub comparison_response_id: Presence<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    Implicit,
    Explicit,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CacheTtl {
    #[serde(rename = "30m")]
    ThirtyMinutes,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CacheDiagnostics {
    CacheHit,
    ComparisonResponseNotFound,
    Unavailable,
    CacheMiss {
        reason: CacheMissReason,
        cache_missed_tokens: u64,
        #[serde(default, skip_serializing_if = "Presence::is_absent")]
        comparison_reusable_tokens: Presence<u64>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMissReason {
    ModelChanged,
    PromptCacheKeyChanged,
    ToolsChanged,
    TextFormatChanged,
    ReasoningEffortChanged,
    VerbosityChanged,
    ContextCompacted,
    InputChanged,
    ServiceTierChanged,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionHints {
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub metadata: Presence<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub service_tier: Presence<ServiceTier>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub safety_identifier: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub user: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt_cache_key: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt_cache_retention: Presence<CacheRetention>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt_cache_options: Presence<CacheOptions>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub max_tool_calls: Presence<u64>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub store: Presence<bool>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub background: Presence<bool>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub previous_response_id: Presence<()>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub conversation: Presence<()>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt: Presence<()>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub moderation: Presence<()>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub context_management: Presence<Vec<()>>,
}
pub(super) const EXEC_FIELDS: &[&str] = &[
    "metadata",
    "service_tier",
    "safety_identifier",
    "user",
    "prompt_cache_key",
    "prompt_cache_retention",
    "prompt_cache_options",
    "max_tool_calls",
    "store",
    "background",
    "previous_response_id",
    "conversation",
    "prompt",
    "moderation",
    "context_management",
];
impl ExecutionHints {
    pub fn validate(&self) -> Result<(), CodecError> {
        if self.store == Presence::Value(true)
            || self.background == Presence::Value(true)
            || self
                .context_management
                .value()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(CodecError::Unsupported("stateful execution".into()));
        }
        for (value, max) in [
            (&self.safety_identifier, 64),
            (&self.user, 256),
            (&self.prompt_cache_key, 256),
        ] {
            if value
                .value()
                .is_some_and(|s| s.chars().count() > max || s.len() > max * 4)
            {
                return Err(CodecError::Limit);
            }
        }
        if self.prompt_cache_options.value().is_some_and(|o| {
            o.comparison_response_id
                .value()
                .is_some_and(|s| s.len() > 256)
        }) {
            return Err(CodecError::Limit);
        }
        if self.metadata.value().is_some_and(|m| {
            m.len() > 16
                || m.iter()
                    .any(|(k, v)| k.chars().count() > 64 || v.chars().count() > 512)
        }) {
            return Err(CodecError::Limit);
        }
        if self.max_tool_calls == Presence::Value(0) {
            return Err(CodecError::Invalid("max_tool_calls"));
        }
        json_size(self, MAX_TEXT_BYTES).map_err(|_| CodecError::Limit)?;
        Ok(())
    }
    pub(super) fn read(o: &Map<String, Value>) -> Result<Self, CodecError> {
        let fields: Map<_, _> = o
            .iter()
            .filter(|(k, _)| EXEC_FIELDS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let hints: Self = serde_json::from_value(Value::Object(fields))
            .map_err(|_| CodecError::Invalid("execution hints"))?;
        hints.validate()?;
        Ok(hints)
    }
    pub(super) fn write(&self, o: &mut Map<String, Value>) -> Result<(), CodecError> {
        self.validate()?;
        let Value::Object(v) =
            serde_json::to_value(self).map_err(|_| CodecError::Invalid("execution hints"))?
        else {
            unreachable!()
        };
        o.extend(v);
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamOptions {
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub include_obfuscation: Presence<bool>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Delivery {
    pub stream: Presence<bool>,
    pub options: Presence<StreamOptions>,
}
impl Delivery {
    pub fn streaming(&self) -> bool {
        self.stream == Presence::Value(true)
    }
    pub fn validate(&self) -> Result<(), CodecError> {
        if self.options.value().is_some() && !self.streaming() {
            return Err(CodecError::Invalid("stream_options without streaming"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContext {
    pub model: String,
    pub delivery: Delivery,
    pub execution: ExecutionHints,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedResponsesRequest {
    pub task: DecodedRequest,
    pub context: RequestContext,
}
impl RequestContext {
    pub fn validate(&self) -> Result<(), CodecError> {
        text(&self.model, "model", 256)?;
        self.delivery.validate()?;
        self.execution.validate()
    }
}
/// `model` is a public binding label, not an upstream address. The caller resolves it outside IR.
/// Decode a bounded raw JSON request, rejecting duplicate keys before constructing IR.
/// Callers must separately bound body collection; this function receives an existing slice.
pub fn decode_request_bytes(bytes: &[u8]) -> Result<DecodedResponsesRequest, CodecError> {
    decode_request(&super::json::decode(bytes)?)
}
/// Decode a pre-parsed value; original duplicate keys and raw byte size cannot be checked here.
pub fn decode_request(v: &Value) -> Result<DecodedResponsesRequest, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    let allowed: Vec<_> = settings::FIELDS
        .iter()
        .chain(EXEC_FIELDS)
        .copied()
        .chain(["input", "model", "stream", "stream_options"])
        .collect();
    fields(o, &allowed)?;
    let context = RequestContext {
        model: string(o, "model")?.into(),
        delivery: Delivery {
            stream: read_presence(o, "stream", |v| {
                v.as_bool().ok_or(CodecError::Invalid("stream"))
            })?,
            options: read_presence(o, "stream_options", |v| {
                serde_json::from_value(v.clone()).map_err(|_| CodecError::Invalid("stream_options"))
            })?,
        },
        execution: ExecutionHints::read(o)?,
    };
    context.validate()?;
    let task: Map<_, _> = o
        .iter()
        .filter(|(k, _)| k.as_str() == "input" || settings::FIELDS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(DecodedResponsesRequest {
        task: super::responses::decode_generation(&Value::Object(task))?,
        context,
    })
}
/// Stateless policy is explicit: omission at ingress never enables provider storage by default.
pub fn encode_request(
    target: &RequestRepresentation<'_>,
    context: &RequestContext,
) -> Result<Value, CodecError> {
    context.validate()?;
    if target.profile != Profile::Responses {
        return Err(CodecError::ProfileMismatch);
    }
    let mut v = super::responses::encode_generation(target)?;
    let o = v.as_object_mut().expect("object");
    context.execution.write(o)?;
    o.insert("store".into(), json!(false));
    o.insert("model".into(), json!(context.model));
    put_presence(o, "stream", &context.delivery.stream, |s| json!(s));
    put_presence(o, "stream_options", &context.delivery.options, |s| json!(s));
    bounded(&v)?;
    Ok(v)
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionEcho {
    pub items: Vec<(ItemId, Item)>,
    pub fidelity: crate::protocol::fidelity::FidelityRecords,
}
impl InstructionEcho {
    fn validate(&self) -> Result<(), CodecError> {
        if self
            .items
            .iter()
            .any(|(_, i)| !matches!(i, Item::Instruction(_)))
        {
            return Err(CodecError::Unsupported("non-instruction echo item".into()));
        }
        if !self.items.is_empty() {
            GenerationRequest::new(self.items.clone(), GenerationControls::default())?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResponseContext {
    pub settings: Option<GenerationSettings>,
    pub instruction_messages: Option<InstructionEcho>,
    pub execution: ExecutionHints,
    pub completed_at: Presence<Number>,
    pub cache_diagnostics: Presence<CacheDiagnostics>,
}
impl ResponseContext {
    /// Required SDK envelope fields are reported facts; the codec must not invent defaults.
    pub fn validate_complete(&self) -> Result<(), CodecError> {
        self.validate()?;
        let s = self
            .settings
            .as_ref()
            .ok_or(CodecError::Invalid("missing reported settings"))?;
        if s.tools.is_none() || s.tool_choice.is_none() || s.parallel_tool_calls.is_none() {
            return Err(CodecError::Invalid("incomplete reported settings"));
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), CodecError> {
        self.execution.validate()?;
        if let Some(s) = &self.settings {
            s.validate()?;
        }
        if let Some(messages) = &self.instruction_messages {
            if self
                .settings
                .as_ref()
                .is_some_and(|s| !s.instructions.is_absent())
            {
                return Err(CodecError::Invalid("conflicting instruction echoes"));
            }
            messages.validate()?;
        }
        if let Some(n) = self.completed_at.value() {
            timestamp(&Value::Number(n.clone()))?;
        }
        Ok(())
    }
    pub(super) fn read(o: &Map<String, Value>) -> Result<Self, CodecError> {
        let mut controls = o.clone();
        let instruction_messages = if let Some(Value::Array(values)) = o.get("instructions") {
            controls.remove("instructions");
            let mut items = Items::default();
            super::responses::decode_items(&mut items, values, false, "completed")?;
            Some(InstructionEcho {
                items: items.items,
                fidelity: items.fidelity,
            })
        } else {
            None
        };
        let s = if settings::FIELDS.iter().any(|k| o.contains_key(*k)) {
            Some(settings::read(&controls)?)
        } else {
            None
        };
        let value = Self {
            settings: s,
            instruction_messages,
            execution: ExecutionHints::read(o)?,
            completed_at: read_presence(o, "completed_at", timestamp)?,
            cache_diagnostics: read_presence(o, "prompt_cache_diagnostics", |v| {
                serde_json::from_value(v.clone())
                    .map_err(|_| CodecError::Invalid("cache diagnostics"))
            })?,
        };
        value.validate()?;
        Ok(value)
    }
    pub(super) fn write(
        &self,
        o: &mut Map<String, Value>,
        completed: bool,
    ) -> Result<(), CodecError> {
        self.validate()?;
        if let Some(s) = &self.settings {
            settings::write(s, o);
        }
        self.execution.write(o)?;
        if let Some(messages) = &self.instruction_messages {
            o.insert(
                "instructions".into(),
                json!(super::responses::encode_items(
                    &messages.items,
                    &messages.fidelity,
                    false
                )),
            );
        }
        if completed {
            put_presence(o, "completed_at", &self.completed_at, |v| {
                Value::Number(v.clone())
            });
        }
        put_presence(
            o,
            "prompt_cache_diagnostics",
            &self.cache_diagnostics,
            |v| json!(v),
        );
        Ok(())
    }
}
/// Decode a bounded raw JSON response with strict syntax and complete envelope validation.
pub fn decode_response_bytes(bytes: &[u8]) -> Result<super::DecodedResponse, CodecError> {
    decode_response(&super::json::decode(bytes)?)
}
/// Full envelope validation, unlike the lower-level task snapshot codec.
/// A pre-parsed value cannot prove original JSON syntax, duplicate-key or raw-byte validity.
pub fn decode_response(v: &Value) -> Result<super::DecodedResponse, CodecError> {
    bounded(v)?;
    validate_complete_response(v)?;
    super::responses::decode_response(v)
}
pub fn encode_response(target: &super::ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    target.metadata.context.validate_complete()?;
    let v = super::responses::encode_response(target)?;
    validate_complete_response(&v)?;
    Ok(v)
}
pub(super) fn validate_complete_response(v: &Value) -> Result<(), CodecError> {
    let o = object(v)?;
    ResponseContext::read(o)?.validate_complete()?;
    if let Some(u) = o.get("usage").filter(|v| !v.is_null())
        && (u
            .pointer("/input_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .is_none()
            || u.pointer("/input_tokens_details/cache_write_tokens")
                .and_then(Value::as_u64)
                .is_none()
            || u.pointer("/output_tokens_details/reasoning_tokens")
                .and_then(Value::as_u64)
                .is_none())
    {
        return Err(CodecError::Invalid("incomplete usage details"));
    }
    if let Some(output) = o.get("output").and_then(Value::as_array) {
        for item in output {
            validate_item_snapshot(item)?;
        }
    }
    Ok(())
}
pub(super) fn validate_stream_payload(v: &Value) -> Result<(), CodecError> {
    if let Some(response) = v.get("response") {
        validate_complete_response(response)?;
    }
    if let Some(item) = v.get("item") {
        validate_item_snapshot(item)?;
    }
    if let Some(part) = v.get("part") {
        validate_part_snapshot(part)?;
    }
    if v.get("type").and_then(Value::as_str) == Some("response.output_text.annotation.added")
        && v.pointer("/annotation/type").and_then(Value::as_str) == Some("file_path")
    {
        return Err(CodecError::Unsupported("file_path annotation event".into()));
    }
    Ok(())
}
pub(super) fn validate_item_snapshot(v: &Value) -> Result<(), CodecError> {
    let o = object(v)?;
    if string(o, "type")? == "message" {
        // Full wire snapshots report identity/lifecycle; only task codecs allow defaults.
        text(string(o, "id")?, "wire item id", 256)?;
        if string(o, "role")? != "assistant" || !o.contains_key("status") {
            return Err(CodecError::Invalid("output message headers"));
        }
        super::responses::status(o, ItemLifecycle::Completed)?;
        let parts = o
            .get("content")
            .and_then(Value::as_array)
            .ok_or(CodecError::Invalid("output content"))?;
        for p in parts {
            validate_part_snapshot(p)?;
        }
    }
    Ok(())
}
pub(super) fn validate_part_snapshot(v: &Value) -> Result<(), CodecError> {
    if v.get("type").and_then(Value::as_str) == Some("output_text")
        && let Some(probs) = v.get("logprobs").filter(|v| !v.is_null())
    {
        for p in super::text::read_logprobs(probs)? {
            if p.bytes.is_none()
                || p.top_logprobs.as_ref().is_none_or(|v| {
                    v.iter()
                        .any(|p| p.token.is_none() || p.logprob.is_none() || p.bytes.is_none())
                })
            {
                return Err(CodecError::Invalid("incomplete static logprobs"));
            }
        }
    }
    Ok(())
}
pub(super) fn timestamp(v: &Value) -> Result<Number, CodecError> {
    match v {
        Value::Number(n) if n.as_f64().is_some_and(|v| v.is_finite() && v >= 0.0) => Ok(n.clone()),
        _ => Err(CodecError::Invalid("timestamp")),
    }
}
pub(super) fn response_fields(o: &Map<String, Value>) -> Result<(), CodecError> {
    let allowed: Vec<_> = settings::FIELDS
        .iter()
        .chain(EXEC_FIELDS)
        .copied()
        .chain([
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
            "prompt_cache_diagnostics",
        ])
        .collect();
    fields(o, &allowed)
}
pub(super) fn write_metadata(
    metadata: &ResponseMetadata,
    o: &mut Map<String, Value>,
    completed: bool,
) -> Result<(), CodecError> {
    metadata.context.write(o, completed)
}
