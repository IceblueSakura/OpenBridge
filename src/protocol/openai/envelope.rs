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
    /// Omitted, explicit false and true stay distinct; `null` is not a bool.
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prewarm: Presence<bool>,
}
impl CacheOptions {
    fn validate(&self) -> Result<(), CodecError> {
        if matches!(self.prewarm, Presence::Null) {
            return Err(CodecError::Invalid("prewarm"));
        }
        if self
            .comparison_response_id
            .value()
            .is_some_and(|s| s.len() > 256)
        {
            return Err(CodecError::Limit);
        }
        Ok(())
    }
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
/// Cache-affinity and user-bucketing hints shared by Responses and Chat bodies.
/// A request hint and a reported echo are separate facts: the codec never converts
/// `prompt_cache_retention` (maximum policy) into `prompt_cache_options.ttl`
/// (minimum lifetime) or aliases `user` onto `prompt_cache_key`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheHints {
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt_cache_key: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt_cache_retention: Presence<CacheRetention>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub prompt_cache_options: Presence<CacheOptions>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub safety_identifier: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub user: Presence<String>,
}
pub(super) const CACHE_FIELDS: &[&str] = &[
    "prompt_cache_key",
    "prompt_cache_retention",
    "prompt_cache_options",
    "safety_identifier",
    "user",
];
impl CacheHints {
    pub fn validate(&self) -> Result<(), CodecError> {
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
        if let Some(options) = self.prompt_cache_options.value() {
            options.validate()?;
        }
        json_size(self, MAX_TEXT_BYTES).map_err(|_| CodecError::Limit)?;
        Ok(())
    }
    pub(super) fn read(o: &Map<String, Value>) -> Result<Self, CodecError> {
        let fields: Map<_, _> = o
            .iter()
            .filter(|(k, _)| CACHE_FIELDS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let hints: Self = serde_json::from_value(Value::Object(fields))
            .map_err(|_| CodecError::Invalid("cache hints"))?;
        hints.validate()?;
        Ok(hints)
    }
    pub(super) fn write(&self, o: &mut Map<String, Value>) -> Result<(), CodecError> {
        self.validate()?;
        let Value::Object(v) =
            serde_json::to_value(self).map_err(|_| CodecError::Invalid("cache hints"))?
        else {
            unreachable!()
        };
        o.extend(v);
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionHints {
    #[serde(flatten)]
    pub cache: CacheHints,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub metadata: Presence<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Presence::is_absent")]
    pub service_tier: Presence<ServiceTier>,
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
        self.cache.validate()?;
        if self.store == Presence::Value(true)
            || self.background == Presence::Value(true)
            || self
                .context_management
                .value()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(CodecError::Unsupported("stateful execution".into()));
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
        Ok(())
    }
    pub(super) fn read(o: &Map<String, Value>) -> Result<Self, CodecError> {
        let fields: Map<_, _> = o
            .iter()
            .filter(|(k, _)| {
                EXEC_FIELDS.contains(&k.as_str()) || CACHE_FIELDS.contains(&k.as_str())
            })
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
    /// Provider-scoped body sections (`client_metadata`), never task semantics.
    pub extensions: crate::protocol::extensions::CustomSections,
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
        .chain(CACHE_FIELDS)
        .copied()
        .chain(["input", "model", "stream", "stream_options"])
        .chain(crate::protocol::extensions::SECTION_NAMES.iter().copied())
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
        extensions: crate::protocol::extensions::CustomSections::read(o)?,
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
    context.extensions.write(o)?;
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
    /// Standard Chat reported fact with no Responses wire position. Presence is
    /// preserved; cross-profile projection rejects instead of dropping it.
    pub system_fingerprint: Presence<String>,
}
impl ResponseContext {
    /// Reported facts are presence-preserving (ADR 0008): absence is a fact and
    /// the codec never invents defaults or request copies. Structural snapshot
    /// completeness lives in `validate_response_snapshot`.
    pub fn validate(&self) -> Result<(), CodecError> {
        self.execution.validate()?;
        if self
            .system_fingerprint
            .value()
            .is_some_and(|fingerprint| fingerprint.is_empty() || fingerprint.len() > 256)
        {
            return Err(CodecError::Invalid("system fingerprint"));
        }
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
            Some(settings::read(&controls, true)?)
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
            // Responses wire has no `system_fingerprint` position; the field is
            // Chat-only and rejected on Responses bodies by `response_fields`.
            system_fingerprint: crate::semantic::value::Presence::Absent,
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
    validate_response_snapshot(v)?;
    let mut decoded = super::responses::decode_response(v)?;
    record_vendor_shapes(Profile::Responses, object(v)?, &mut decoded.fidelity)?;
    Ok(decoded)
}
pub fn encode_response(target: &super::ResponseRepresentation<'_>) -> Result<Value, CodecError> {
    target.metadata.context.validate()?;
    let mut v = super::responses::encode_response(target)?;
    write_response_extras(
        target.fidelity,
        target.profile,
        v.as_object_mut().expect("object"),
    );
    validate_response_snapshot(&v)?;
    Ok(v)
}
/// Vendor adaptation (ADR 0008): validate-then-drop derived views, then record
/// classified response extras into fidelity for same-origin re-encode.
pub(crate) fn record_vendor_shapes(
    profile: Profile,
    o: &Map<String, Value>,
    fidelity: &mut crate::protocol::fidelity::FidelityRecords,
) -> Result<(), CodecError> {
    if let Some(view) = o.get("output_text").filter(|v| !v.is_null()) {
        let view = view.as_str().ok_or(CodecError::Invalid("output_text"))?;
        // The equality rule applies where the snapshot claims finality: a
        // non-terminal snapshot may report a partial view while items stream.
        let terminal = o
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|s| matches!(s, "completed" | "incomplete" | "failed" | "cancelled"));
        if terminal {
            let mut authoritative = String::new();
            if let Some(items) = o.get("output").and_then(Value::as_array) {
                for item in items {
                    if item.get("type").and_then(Value::as_str) != Some("message") {
                        continue;
                    }
                    if let Some(parts) = item.get("content").and_then(Value::as_array) {
                        for part in parts {
                            if part.get("type").and_then(Value::as_str) == Some("output_text")
                                && let Some(text) = part.get("text").and_then(Value::as_str)
                            {
                                authoritative.push_str(text);
                            }
                        }
                    }
                }
            }
            if view != authoritative {
                return Err(CodecError::Invalid("output_text"));
            }
        }
    }
    fidelity.record_response_extras(profile, o)
}
/// Same-origin re-encode of classified vendor extras; never standard semantics.
pub(crate) fn write_response_extras(
    fidelity: &crate::protocol::fidelity::FidelityRecords,
    profile: Profile,
    o: &mut Map<String, Value>,
) {
    for name in crate::protocol::fidelity::declared_response_extras(profile) {
        if let Some(value) = fidelity.response_extras().get(*name) {
            o.insert((*name).to_string(), value.clone());
        }
    }
}
/// Structural snapshot validation (ADR 0008): reported facts are presence-preserving
/// and typed by the task codec; this layer keeps the structural requirements.
pub(super) fn validate_response_snapshot(v: &Value) -> Result<(), CodecError> {
    let o = object(v)?;
    ResponseContext::read(o)?.validate()?;
    // A complete Response snapshot requires the output array; an absent value is
    // never the explicit empty array and no lower layer may backfill it.
    for item in o
        .get("output")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("output"))?
    {
        validate_item_snapshot(item)?;
    }
    Ok(())
}
pub(super) fn validate_stream_payload(v: &Value) -> Result<(), CodecError> {
    // Every complete Responses SSE event carries the required integer sequence
    // number; event ordering never substitutes for its presence.
    if object(v)?
        .get("sequence_number")
        .and_then(Value::as_u64)
        .is_none()
    {
        return Err(CodecError::Invalid("sequence"));
    }
    if let Some(response) = v.get("response") {
        validate_response_snapshot(response)?;
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
    } else if matches!(string(o, "type")?, "reasoning" | "configuration_update") {
        // These snapshots require identity; task codecs still assign fresh identity.
        text(string(o, "id")?, "wire item id", 256)?;
    }
    Ok(())
}
pub(super) fn validate_part_snapshot(v: &Value) -> Result<(), CodecError> {
    if v.get("type").and_then(Value::as_str) == Some("output_text") {
        let annotations = v.get("annotations").filter(|a| !a.is_null());
        if !annotations.is_some_and(Value::is_array) {
            return Err(CodecError::Invalid("annotations"));
        }
    }
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
        .chain(CACHE_FIELDS)
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
            // Classified vendor extras (ADR 0008), captured into fidelity.
            "content_filters",
            "frequency_penalty",
            "presence_penalty",
            // Derived view, validated then dropped.
            "output_text",
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
