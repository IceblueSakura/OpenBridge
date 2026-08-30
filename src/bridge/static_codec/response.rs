//! Pure non-stream response decoding and target encoding through static Generation IR.

use std::collections::BTreeSet;

use base64::Engine;
use bytes::Bytes;
use serde_json::{Map, Value, json};

use crate::{
    core::{ApiProtocol, ReasoningOutput},
    ir::generation::{
        AudioResource, BoundedBytes, Candidate, CandidateId, ChangeAuthorization, ChangeKind,
        ChangeReason, ContentPart, FinishReason, GenerationResponse, InlineResource, ItemId,
        JsonObject, OpaqueExposure, OpaqueKind, OpaqueState, OutputItem, ProviderNamespace,
        ReasoningItem, ReasoningPart, Resource, ResourceSource, ResponseId, ResponseMessage,
        ResponseStatus, SemanticChange, SemanticPath, TextValue, ToolCall, ToolInput, ToolName,
        Transform, Usage,
    },
};

use super::{StaticCodecError, WireResponse};

/// Closed private wire DTO produced only after canonical response lowering succeeds.
pub(super) enum TargetResponse {
    Chat(Map<String, Value>),
    Responses(Map<String, Value>),
}

/// Decodes one successful complete upstream response into canonical response semantics.
pub(super) fn decode_response(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
    reasoning_output: ReasoningOutput,
    max_bytes: usize,
) -> Result<WireResponse, StaticCodecError> {
    match protocol {
        ApiProtocol::ChatCompletions => decode_chat_response(source, reasoning_output, max_bytes),
        ApiProtocol::Responses => decode_responses_response(source, reasoning_output, max_bytes),
    }
}

/// Lowers one complete canonical response into a private downstream DTO.
pub(super) fn lower_response(
    protocol: ApiProtocol,
    response: &WireResponse,
    public_model: &str,
    reasoning_output: ReasoningOutput,
) -> Result<Transform<TargetResponse>, StaticCodecError> {
    if response.semantic.status() != ResponseStatus::Completed
        || response.semantic.candidates().len() != 1
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let value = match protocol {
        ApiProtocol::ChatCompletions => {
            encode_chat_response(response, public_model, reasoning_output)?
        }
        ApiProtocol::Responses => {
            encode_responses_response(response, public_model, reasoning_output)?
        }
    };
    let object = value
        .as_object()
        .cloned()
        .ok_or(StaticCodecError::InvalidShape)?;
    let target = match protocol {
        ApiProtocol::ChatCompletions => TargetResponse::Chat(object),
        ApiProtocol::Responses => TargetResponse::Responses(object),
    };
    Ok(Transform::new(
        target,
        vec![SemanticChange::new(
            SemanticPath::root(),
            ChangeKind::Normalized,
            ChangeReason::ProtocolNormalized,
            ChangeAuthorization::default(),
        )],
    ))
}

/// Encodes a lowered private response DTO into compact JSON bytes.
pub(super) fn encode_response(target: TargetResponse) -> Result<Bytes, StaticCodecError> {
    let object = match target {
        TargetResponse::Chat(object) | TargetResponse::Responses(object) => object,
    };
    serde_json::to_vec(&Value::Object(object))
        .map(Bytes::from)
        .map_err(|_| StaticCodecError::InvalidShape)
}

fn decode_chat_response(
    source: &Map<String, Value>,
    reasoning_output: ReasoningOutput,
    max_bytes: usize,
) -> Result<WireResponse, StaticCodecError> {
    if source.get("object").and_then(Value::as_str) != Some("chat.completion") {
        return Err(StaticCodecError::InvalidShape);
    }
    let source_id = required_string(source, "id")?;
    let choices = source
        .get("choices")
        .and_then(Value::as_array)
        .ok_or(StaticCodecError::InvalidShape)?;
    if choices.len() != 1 {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let choice = choices[0]
        .as_object()
        .ok_or(StaticCodecError::InvalidShape)?;
    if choice.get("index").and_then(Value::as_u64) != Some(0) {
        return Err(StaticCodecError::InvalidShape);
    }
    let finish = decode_finish(choice.get("finish_reason"))?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or(StaticCodecError::InvalidShape)?;
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(StaticCodecError::InvalidShape);
    }

    let suffix = source_id.strip_prefix("chatcmpl_").unwrap_or(&source_id);
    let mut output = Vec::new();
    if let Some(reasoning) = optional_nonempty_string(message, "reasoning_content")? {
        if !reasoning_output.is_readable() {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        output.push(OutputItem::Reasoning(
            ReasoningItem::new(
                item_id(format!("rs_{suffix}"), max_bytes)?,
                vec![ReasoningPart::Visible(text_value(reasoning, max_bytes)?)],
                None,
            )
            .map_err(|_| StaticCodecError::InvalidShape)?,
        ));
    }
    let mut message_content = Vec::new();
    if let Some(content) = message.get("content").filter(|value| !value.is_null()) {
        let content = content.as_str().ok_or(StaticCodecError::InvalidShape)?;
        if !content.is_empty() {
            message_content.push(ContentPart::text(text_value(
                content.to_owned(),
                max_bytes,
            )?));
        }
    }
    if let Some(audio) = message.get("audio").filter(|value| !value.is_null()) {
        let audio = audio.as_object().ok_or(StaticCodecError::InvalidShape)?;
        let data = required_string(audio, "data")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&data))
            .map_err(|_| StaticCodecError::InvalidShape)?;
        let bytes =
            BoundedBytes::new(bytes, max_bytes).map_err(StaticCodecError::from_validation)?;
        let inline = InlineResource::new(bytes).map_err(StaticCodecError::from_validation)?;
        message_content.push(ContentPart::Resource(Resource::Audio(AudioResource::new(
            ResourceSource::Inline(inline),
            None,
        ))));
    }
    if !message_content.is_empty() {
        output.push(OutputItem::Message(
            ResponseMessage::new(
                item_id(format!("msg_{suffix}"), max_bytes)?,
                message_content,
                None,
            )
            .map_err(|_| StaticCodecError::InvalidShape)?,
        ));
    }
    if let Some(calls) = message.get("tool_calls") {
        let calls = calls.as_array().ok_or(StaticCodecError::InvalidShape)?;
        let mut item_ids = BTreeSet::new();
        for (ordinal, call) in calls.iter().enumerate() {
            let call = call.as_object().ok_or(StaticCodecError::InvalidShape)?;
            if call.get("type").and_then(Value::as_str) != Some("function") {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
            let function = call
                .get("function")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            let call_id = required_string(call, "id")?;
            let preferred = allocate_item_id(&call_id, ordinal + 1, &mut item_ids);
            output.push(OutputItem::ToolCall(tool_call(
                preferred,
                call_id,
                required_string(function, "name")?,
                required_string(function, "arguments")?,
                max_bytes,
            )?));
        }
    }
    if output.is_empty() {
        return Err(StaticCodecError::InvalidShape);
    }
    let candidate = Candidate::new(
        candidate_id("candidate_0", max_bytes)?,
        output,
        Some(finish),
    )
    .map_err(|_| StaticCodecError::InvalidShape)?;
    let response = GenerationResponse::new(
        response_id(format!("response_{suffix}"), max_bytes)?,
        vec![candidate],
        ResponseStatus::Completed,
        decode_chat_usage(source.get("usage"))?,
        Vec::new(),
    )
    .map_err(|_| StaticCodecError::InvalidShape)?;
    Ok(WireResponse {
        semantic: response,
        source_id,
    })
}

fn decode_responses_response(
    source: &Map<String, Value>,
    reasoning_output: ReasoningOutput,
    max_bytes: usize,
) -> Result<WireResponse, StaticCodecError> {
    if source.get("object").and_then(Value::as_str) != Some("response")
        || source.get("status").and_then(Value::as_str) != Some("completed")
    {
        return Err(StaticCodecError::InvalidShape);
    }
    let source_id = required_string(source, "id")?;
    let items = source
        .get("output")
        .and_then(Value::as_array)
        .ok_or(StaticCodecError::InvalidShape)?;
    let mut output = Vec::new();
    let mut has_tool_call = false;
    for item in items {
        let item = item.as_object().ok_or(StaticCodecError::InvalidShape)?;
        let status = item.get("status").filter(|value| !value.is_null());
        if status.is_some_and(|value| value.as_str() != Some("completed")) {
            return Err(StaticCodecError::InvalidShape);
        }
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                if status.is_none() {
                    return Err(StaticCodecError::InvalidShape);
                }
                let id = required_string(item, "id")?;
                let content = decode_responses_message_content(item, max_bytes)?;
                output.push(OutputItem::Message(
                    ResponseMessage::new(item_id(id, max_bytes)?, content, None)
                        .map_err(|_| StaticCodecError::InvalidShape)?,
                ));
            }
            Some("reasoning") => {
                let id = required_string(item, "id")?;
                let encrypted = item
                    .get("encrypted_content")
                    .filter(|value| !value.is_null())
                    .map(|value| value.as_str().ok_or(StaticCodecError::InvalidShape))
                    .transpose()?;
                if encrypted.is_some_and(|value| !value.is_empty())
                    && !reasoning_output.is_readable()
                {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                let mut parts = Vec::new();
                decode_reasoning_parts(item, "content", "reasoning_text", max_bytes, &mut parts)?;
                decode_reasoning_parts(item, "summary", "summary_text", max_bytes, &mut parts)?;
                if let Some(encrypted) = encrypted.filter(|value| !value.is_empty()) {
                    let namespace = ProviderNamespace::new("openai.responses", max_bytes)
                        .map_err(StaticCodecError::from_validation)?;
                    let payload = BoundedBytes::from_slice(encrypted.as_bytes(), max_bytes)
                        .map_err(StaticCodecError::from_validation)?;
                    let state = OpaqueState::new(
                        namespace,
                        OpaqueKind::EncryptedContent,
                        payload,
                        None,
                        OpaqueExposure::InternalOnly,
                    )
                    .map_err(StaticCodecError::from_validation)?;
                    parts.push(ReasoningPart::Opaque(state));
                }
                if !parts.is_empty() {
                    output.push(OutputItem::Reasoning(
                        ReasoningItem::new(item_id(id, max_bytes)?, parts, None)
                            .map_err(|_| StaticCodecError::InvalidShape)?,
                    ));
                }
            }
            Some("function_call") => {
                has_tool_call = true;
                output.push(OutputItem::ToolCall(tool_call(
                    required_string(item, "id")?,
                    required_string(item, "call_id")?,
                    required_string(item, "name")?,
                    required_string(item, "arguments")?,
                    max_bytes,
                )?));
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    if output.is_empty() {
        return Err(StaticCodecError::InvalidShape);
    }
    let candidate = Candidate::new(
        candidate_id("candidate_0", max_bytes)?,
        output,
        Some(if has_tool_call {
            FinishReason::ToolCalls
        } else {
            FinishReason::Stop
        }),
    )
    .map_err(|_| StaticCodecError::InvalidShape)?;
    let response = GenerationResponse::new(
        response_id(source_id.clone(), max_bytes)?,
        vec![candidate],
        ResponseStatus::Completed,
        decode_responses_usage(source.get("usage"))?,
        Vec::new(),
    )
    .map_err(|_| StaticCodecError::InvalidShape)?;
    Ok(WireResponse {
        semantic: response,
        source_id,
    })
}

fn encode_chat_response(
    response: &WireResponse,
    public_model: &str,
    reasoning_output: ReasoningOutput,
) -> Result<Value, StaticCodecError> {
    let candidate = &response.semantic.candidates()[0];
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_calls = Vec::new();
    for item in candidate.output() {
        match item {
            OutputItem::Message(message) => text.push_str(&flatten_text(message.content())?),
            OutputItem::Reasoning(item) => {
                if !reasoning_output.is_readable() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                for part in item.parts() {
                    match part {
                        ReasoningPart::Visible(value) | ReasoningPart::Summary(value) => {
                            reasoning.push_str(value.as_str());
                        }
                        // Keep replay-only Provider state inside IR; never expose it as readable Chat output.
                        ReasoningPart::Opaque(_) => {}
                    }
                }
            }
            OutputItem::ToolCall(call) => tool_calls.push(json!({
                "function": {
                    "arguments": function_arguments(call)?,
                    "name": call.tool().as_str()
                },
                "id": call.call_id().as_str(),
                "type": "function"
            })),
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    let mut message = Map::new();
    message.insert("role".to_owned(), Value::String("assistant".to_owned()));
    if !reasoning.is_empty() {
        message.insert("reasoning_content".to_owned(), Value::String(reasoning));
    }
    let finish_reason = if tool_calls.is_empty() {
        message.insert(
            "content".to_owned(),
            if text.is_empty() {
                Value::Null
            } else {
                Value::String(text)
            },
        );
        encode_chat_finish(candidate.finish())?
    } else {
        message.insert(
            "content".to_owned(),
            if text.is_empty() {
                Value::Null
            } else {
                Value::String(text)
            },
        );
        message.insert("tool_calls".to_owned(), Value::Array(tool_calls));
        "tool_calls"
    };
    let mut result = json!({
        "choices": [{"finish_reason": finish_reason, "index": 0, "message": message}],
        "id": map_id(&response.source_id, "resp_", "chatcmpl_"),
        "model": public_model,
        "object": "chat.completion"
    });
    if let Some(usage) = response.semantic.usage() {
        let mut encoded_usage = json!({
            "completion_tokens": usage.output_tokens(),
            "prompt_tokens": usage.input_tokens(),
            "total_tokens": usage.total_tokens()
        });
        if let Some(reasoning_tokens) = usage.reasoning_tokens() {
            encoded_usage["completion_tokens_details"] =
                json!({"reasoning_tokens": reasoning_tokens});
        }
        if let Some(cached_tokens) = usage.cached_input_tokens() {
            encoded_usage["prompt_tokens_details"] = json!({"cached_tokens": cached_tokens});
        }
        result["usage"] = encoded_usage;
    }
    Ok(result)
}

fn encode_responses_response(
    response: &WireResponse,
    public_model: &str,
    reasoning_output: ReasoningOutput,
) -> Result<Value, StaticCodecError> {
    let candidate = &response.semantic.candidates()[0];
    let mut output = Vec::new();
    for item in candidate.output() {
        match item {
            OutputItem::Message(message) => output.push(json!({
                "content": [{
                    "annotations": [],
                    "text": flatten_text(message.content())?,
                    "type": "output_text"
                }],
                "id": message.id().as_str(),
                "role": "assistant",
                "status": "completed",
                "type": "message"
            })),
            OutputItem::Reasoning(item) => {
                if !reasoning_output.is_readable() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                let mut content = Vec::new();
                let mut summary = Vec::new();
                for part in item.parts() {
                    match part {
                        ReasoningPart::Visible(value) => content.push(json!({
                            "text": value.as_str(), "type": "reasoning_text"
                        })),
                        ReasoningPart::Summary(value) => summary.push(json!({
                            "text": value.as_str(), "type": "summary_text"
                        })),
                        // A cross-protocol Chat source cannot produce Responses-owned replay state.
                        ReasoningPart::Opaque(_) => {}
                    }
                }
                output.push(json!({
                    "content": content,
                    "id": item.id().as_str(),
                    "status": "completed",
                    "summary": summary,
                    "type": "reasoning"
                }));
            }
            OutputItem::ToolCall(call) => output.push(json!({
                "arguments": function_arguments(call)?,
                "call_id": call.call_id().as_str(),
                "id": call.id().as_str(),
                "name": call.tool().as_str(),
                "status": "completed",
                "type": "function_call"
            })),
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    let mut result = json!({
        "id": map_id(&response.source_id, "chatcmpl_", "resp_"),
        "model": public_model,
        "object": "response",
        "output": output,
        "status": "completed"
    });
    if let Some(usage) = response.semantic.usage() {
        let mut encoded_usage = json!({
            "input_tokens": usage.input_tokens(),
            "output_tokens": usage.output_tokens(),
            "total_tokens": usage.total_tokens()
        });
        if let Some(reasoning_tokens) = usage.reasoning_tokens() {
            encoded_usage["output_tokens_details"] = json!({"reasoning_tokens": reasoning_tokens});
        }
        if let Some(cached_tokens) = usage.cached_input_tokens() {
            encoded_usage["input_tokens_details"] = json!({"cached_tokens": cached_tokens});
        }
        result["usage"] = encoded_usage;
    }
    Ok(result)
}

fn decode_responses_message_content(
    item: &Map<String, Value>,
    max_bytes: usize,
) -> Result<Vec<ContentPart>, StaticCodecError> {
    if item.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(StaticCodecError::InvalidShape);
    }
    let parts = item
        .get("content")
        .and_then(Value::as_array)
        .ok_or(StaticCodecError::InvalidShape)?;
    parts
        .iter()
        .map(|part| {
            let part = part.as_object().ok_or(StaticCodecError::InvalidShape)?;
            if part.get("type").and_then(Value::as_str) != Some("output_text") {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
            if let Some(annotations) = part.get("annotations") {
                let annotations = annotations
                    .as_array()
                    .ok_or(StaticCodecError::InvalidShape)?;
                if !annotations.is_empty() {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
            }
            Ok(ContentPart::text(text_value(
                required_string(part, "text")?,
                max_bytes,
            )?))
        })
        .collect()
}

fn decode_reasoning_parts(
    item: &Map<String, Value>,
    field: &str,
    expected: &str,
    max_bytes: usize,
    target: &mut Vec<ReasoningPart>,
) -> Result<(), StaticCodecError> {
    let Some(parts) = item.get(field) else {
        return Ok(());
    };
    let parts = parts.as_array().ok_or(StaticCodecError::InvalidShape)?;
    for part in parts {
        let part = part.as_object().ok_or(StaticCodecError::InvalidShape)?;
        if part.get("type").and_then(Value::as_str) != Some(expected) {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        let value = text_value(required_string(part, "text")?, max_bytes)?;
        target.push(if field == "content" {
            ReasoningPart::Visible(value)
        } else {
            ReasoningPart::Summary(value)
        });
    }
    Ok(())
}

fn decode_finish(value: Option<&Value>) -> Result<FinishReason, StaticCodecError> {
    match value.and_then(Value::as_str) {
        Some("stop") => Ok(FinishReason::Stop),
        Some("tool_calls") => Ok(FinishReason::ToolCalls),
        Some("length") => Ok(FinishReason::Length),
        Some("content_filter") => Ok(FinishReason::ContentFilter),
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn encode_chat_finish(finish: Option<&FinishReason>) -> Result<&'static str, StaticCodecError> {
    match finish {
        Some(FinishReason::Stop) => Ok("stop"),
        Some(FinishReason::Length) => Ok("length"),
        Some(FinishReason::ToolCalls) => Ok("tool_calls"),
        Some(FinishReason::ContentFilter) => Ok("content_filter"),
        Some(FinishReason::Extension(_)) | None => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn decode_chat_usage(value: Option<&Value>) -> Result<Option<Usage>, StaticCodecError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let usage = value.as_object().ok_or(StaticCodecError::InvalidShape)?;
    let input = required_usage_u64(usage, "prompt_tokens")?;
    let output = required_usage_u64(usage, "completion_tokens")?;
    let total = required_usage_u64(usage, "total_tokens")?;
    validate_usage_total(input, output, total)?;
    Ok(Some(Usage::new(
        Some(input),
        Some(output),
        Some(total),
        optional_detail_object(usage, "completion_tokens_details")?
            .map(|details| optional_u64(details, "reasoning_tokens"))
            .transpose()?
            .flatten(),
        optional_detail_object(usage, "prompt_tokens_details")?
            .map(|details| optional_u64(details, "cached_tokens"))
            .transpose()?
            .flatten(),
    )))
}

fn decode_responses_usage(value: Option<&Value>) -> Result<Option<Usage>, StaticCodecError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let usage = value.as_object().ok_or(StaticCodecError::InvalidShape)?;
    let input = required_usage_u64(usage, "input_tokens")?;
    let output = required_usage_u64(usage, "output_tokens")?;
    let total = required_usage_u64(usage, "total_tokens")?;
    validate_usage_total(input, output, total)?;
    Ok(Some(Usage::new(
        Some(input),
        Some(output),
        Some(total),
        optional_detail_object(usage, "output_tokens_details")?
            .map(|details| optional_u64(details, "reasoning_tokens"))
            .transpose()?
            .flatten(),
        optional_detail_object(usage, "input_tokens_details")?
            .map(|details| optional_u64(details, "cached_tokens"))
            .transpose()?
            .flatten(),
    )))
}

fn optional_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, StaticCodecError> {
    object
        .get(field)
        .filter(|value| !value.is_null())
        .map(|value| value.as_u64().ok_or(StaticCodecError::InvalidShape))
        .transpose()
}

fn optional_detail_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a Map<String, Value>>, StaticCodecError> {
    match object.get(field) {
        None => Ok(None),
        Some(value) => value
            .as_object()
            .map(Some)
            .ok_or(StaticCodecError::InvalidShape),
    }
}

fn required_usage_u64(object: &Map<String, Value>, field: &str) -> Result<u64, StaticCodecError> {
    optional_u64(object, field)?.ok_or(StaticCodecError::InvalidShape)
}

fn validate_usage_total(input: u64, output: u64, total: u64) -> Result<(), StaticCodecError> {
    if input.checked_add(output) == Some(total) {
        Ok(())
    } else {
        Err(StaticCodecError::InvalidShape)
    }
}

fn function_arguments(call: &ToolCall) -> Result<String, StaticCodecError> {
    match call.input() {
        ToolInput::Function(arguments) => Ok(Value::Object(arguments.as_map().clone()).to_string()),
        ToolInput::Server(_) | ToolInput::Extension(_) => {
            Err(StaticCodecError::UnsupportedSemantics)
        }
    }
}

fn tool_call(
    item: String,
    call: String,
    name: String,
    arguments: String,
    max_bytes: usize,
) -> Result<ToolCall, StaticCodecError> {
    let arguments: Value =
        serde_json::from_str(&arguments).map_err(|_| StaticCodecError::InvalidToolArguments)?;
    let arguments = JsonObject::new(arguments, max_bytes)
        .map_err(|_| StaticCodecError::InvalidToolArguments)?;
    Ok(ToolCall::new(
        item_id(item, max_bytes)?,
        crate::ir::generation::CallId::new(call, max_bytes)
            .map_err(|_| StaticCodecError::InvalidShape)?,
        ToolName::new(name, max_bytes).map_err(StaticCodecError::from_validation)?,
        ToolInput::Function(arguments),
        None,
    ))
}

fn flatten_text(content: &[ContentPart]) -> Result<String, StaticCodecError> {
    let mut text = String::new();
    for part in content {
        match part {
            ContentPart::Text(value) => text.push_str(value.text().as_str()),
            ContentPart::Resource(_) => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    Ok(text)
}

fn allocate_item_id(call_id: &str, ordinal: usize, used: &mut BTreeSet<String>) -> String {
    let suffix = call_id
        .rsplit_once('_')
        .map_or(call_id, |(_, suffix)| suffix);
    let preferred = format!("fc_tool_{suffix}");
    if used.insert(preferred.clone()) {
        return preferred;
    }
    let unique = format!("fc_tool_{ordinal}_{suffix}");
    used.insert(unique.clone());
    unique
}

fn map_id(id: &str, from: &str, to: &str) -> String {
    format!("{to}{}", id.strip_prefix(from).unwrap_or(id))
}

fn required_string(object: &Map<String, Value>, field: &str) -> Result<String, StaticCodecError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(StaticCodecError::InvalidShape)
}

fn optional_nonempty_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, StaticCodecError> {
    let Some(value) = object.get(field).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let value = value.as_str().ok_or(StaticCodecError::InvalidShape)?;
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

fn text_value(value: String, max_bytes: usize) -> Result<TextValue, StaticCodecError> {
    TextValue::new(value, max_bytes).map_err(StaticCodecError::from_validation)
}

fn response_id(value: String, max_bytes: usize) -> Result<ResponseId, StaticCodecError> {
    ResponseId::new(value, max_bytes).map_err(|_| StaticCodecError::InvalidShape)
}

fn candidate_id(value: &str, max_bytes: usize) -> Result<CandidateId, StaticCodecError> {
    CandidateId::new(value.to_owned(), max_bytes).map_err(|_| StaticCodecError::InvalidShape)
}

fn item_id(value: String, max_bytes: usize) -> Result<ItemId, StaticCodecError> {
    ItemId::new(value, max_bytes).map_err(|_| StaticCodecError::InvalidShape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_decoder_keeps_opaque_reasoning_in_static_ir() {
        let source = json!({
            "id": "resp_opaque",
            "object": "response",
            "status": "completed",
            "output": [{
                "id": "rs_opaque",
                "type": "reasoning",
                "status": "completed",
                "encrypted_content": "opaque-state",
                "summary": [{"type": "summary_text", "text": "summary"}]
            }]
        });
        let source = source.as_object().expect("fixture must be an object");

        let decoded = decode_response(
            ApiProtocol::Responses,
            source,
            ReasoningOutput::Summary,
            1024,
        )
        .expect("supported reasoning response must decode");
        let OutputItem::Reasoning(reasoning) = &decoded.semantic.candidates()[0].output()[0] else {
            panic!("reasoning item must remain distinct");
        };

        assert!(
            reasoning
                .parts()
                .iter()
                .any(|part| matches!(part, ReasoningPart::Opaque(_)))
        );
    }
}
