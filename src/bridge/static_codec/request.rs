//! Pure request decoding and target encoding through the static Generation IR.

use std::collections::BTreeSet;

use base64::Engine;
use bytes::Bytes;
use serde_json::{Map, Value, json};

use crate::{
    core::ApiProtocol,
    ir::generation::{
        AudioResource, BoundedBytes, CacheDirective, CacheKey, ChangeAuthorization, ChangeKind,
        ChangeReason, ContentPart, FileResource, FunctionTool, GenerationControls,
        GenerationRequest, ImageDetail, ImageResource, InlineResource, InputItem, Instruction,
        InstructionAuthority, InstructionOrigin, ItemId, JsonObject, JsonSchema, Message,
        MessageRole, OutputConstraint, ParallelToolCalls, ProviderServerTool, ProviderToolProfile,
        ReasoningEffort, ReasoningItem, ReasoningPart, ReasoningRequest, ReasoningSummary,
        RequestState, Resource, ResourceSource, SemanticChange, SemanticPath, TextValue, ToolCall,
        ToolChoice, ToolDefinition, ToolExecutor, ToolInput, ToolKind, ToolName, ToolOrigin,
        ToolOutput, ToolResult, ToolResultStatus, ToolVisibility, Transform, UrlValue,
        lower_provider_server_tool,
    },
};

use super::{StaticCodecError, WireRequest};

/// Closed private wire DTO produced only after canonical request lowering succeeds.
pub(super) enum TargetRequest {
    Chat(Map<String, Value>),
    Responses(Map<String, Value>),
}

/// Decodes one accepted Chat or Responses request into canonical semantics plus delivery metadata.
pub(super) fn decode_request(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
    max_bytes: usize,
) -> Result<WireRequest, StaticCodecError> {
    let stream = optional_bool(source, "stream")?;
    let input = match protocol {
        ApiProtocol::ChatCompletions => decode_chat_input(source, max_bytes)?,
        ApiProtocol::Responses => decode_responses_input(source, max_bytes)?,
    };
    let tools = decode_tools(protocol, source, max_bytes)?;
    let tool_choice = decode_tool_choice(protocol, source.get("tool_choice"), &tools, max_bytes)?;
    let parallel = decode_parallel(source.get("parallel_tool_calls"), tools.is_empty())?;

    let mut request = GenerationRequest::new(input).map_err(StaticCodecError::from_validation)?;
    request = request
        .with_tools(tools, tool_choice, parallel)
        .map_err(StaticCodecError::from_validation)?;
    request = request
        .with_controls(decode_controls(protocol, source, parallel)?)
        .map_err(StaticCodecError::from_validation)?;
    request = request.with_output(decode_output(protocol, source, max_bytes)?);
    request = request.with_reasoning(decode_reasoning(protocol, source)?);
    request = request.with_state(decode_state(source, max_bytes)?);

    Ok(WireRequest {
        semantic: request,
        source: source.clone(),
        stream,
        service_tier: source
            .get("service_tier")
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or(StaticCodecError::InvalidShape)
            })
            .transpose()?,
    })
}

/// Lowers canonical request semantics into one concrete private target DTO.
pub(super) fn lower_request(
    protocol: ApiProtocol,
    request: &WireRequest,
    upstream_model: &str,
    reasoning_summary: bool,
    provider_profile: Option<&ProviderToolProfile>,
) -> Result<Transform<TargetRequest>, StaticCodecError> {
    let mut target = Map::new();
    target.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    match protocol {
        ApiProtocol::ChatCompletions => {
            target.insert(
                "messages".to_owned(),
                Value::Array(encode_chat_input(request.semantic.input())?),
            );
        }
        ApiProtocol::Responses => {
            let (instructions, input) = encode_responses_input(
                request.semantic.input(),
                request.semantic.tools().is_empty(),
                request.stream.unwrap_or(false),
            )?;
            target.insert("input".to_owned(), input);
            if let Some(instructions) = instructions {
                target.insert("instructions".to_owned(), Value::String(instructions));
            }
            target.insert("store".to_owned(), Value::Bool(false));
        }
    }
    target.insert(
        "stream".to_owned(),
        Value::Bool(request.stream.unwrap_or(false)),
    );
    encode_controls(protocol, request.semantic.controls(), &mut target);
    encode_tools(protocol, &request.semantic, provider_profile, &mut target)?;
    encode_output(protocol, request.semantic.output(), &mut target);
    encode_reasoning(
        protocol,
        request.semantic.reasoning(),
        reasoning_summary,
        &mut target,
    )?;
    if let Some(cache) = request.semantic.state().cache()
        && let Some(key) = cache.key()
    {
        target.insert(
            "prompt_cache_key".to_owned(),
            Value::String(key.as_str().to_owned()),
        );
    }
    if let Some(service_tier) = &request.service_tier {
        target.insert(
            "service_tier".to_owned(),
            Value::String(service_tier.clone()),
        );
    }
    let target = match protocol {
        ApiProtocol::ChatCompletions => TargetRequest::Chat(target),
        ApiProtocol::Responses => TargetRequest::Responses(target),
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

/// Encodes a lowered private target DTO into compact JSON bytes.
pub(super) fn encode_request(target: TargetRequest) -> Result<Bytes, StaticCodecError> {
    let object = match target {
        TargetRequest::Chat(object) | TargetRequest::Responses(object) => object,
    };
    serde_json::to_vec(&Value::Object(object))
        .map(Bytes::from)
        .map_err(|_| StaticCodecError::InvalidShape)
}

/// Rejects nested source fields that canonical IR cannot reproduce across protocols.
pub(super) fn validate_bridge_source(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), StaticCodecError> {
    match protocol {
        ApiProtocol::ChatCompletions => validate_chat_bridge_source(source),
        ApiProtocol::Responses => validate_responses_bridge_source(source),
    }?;
    validate_tool_bridge_source(protocol, source)
}

fn validate_tool_bridge_source(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), StaticCodecError> {
    if let Some(tools) = source.get("tools") {
        let tools = tools.as_array().ok_or(StaticCodecError::InvalidShape)?;
        for tool in tools {
            let tool = tool.as_object().ok_or(StaticCodecError::InvalidShape)?;
            match protocol {
                ApiProtocol::ChatCompletions => {
                    ensure_only_fields(tool, &["type", "function"])?;
                    let function = tool
                        .get("function")
                        .and_then(Value::as_object)
                        .ok_or(StaticCodecError::InvalidShape)?;
                    ensure_only_fields(function, &["name", "description", "parameters", "strict"])?;
                }
                ApiProtocol::Responses => ensure_only_fields(
                    tool,
                    &["type", "name", "description", "parameters", "strict"],
                )?,
            }
        }
    }
    let Some(choice) = source.get("tool_choice").filter(|value| value.is_object()) else {
        return Ok(());
    };
    let choice = choice.as_object().ok_or(StaticCodecError::InvalidShape)?;
    match protocol {
        ApiProtocol::ChatCompletions => {
            ensure_only_fields(choice, &["type", "function"])?;
            let function = choice
                .get("function")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            ensure_only_fields(function, &["name"])
        }
        ApiProtocol::Responses => ensure_only_fields(choice, &["type", "name"]),
    }
}

fn validate_chat_bridge_source(source: &Map<String, Value>) -> Result<(), StaticCodecError> {
    let messages = source
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(StaticCodecError::InvalidShape)?;
    for message in messages {
        let message = message.as_object().ok_or(StaticCodecError::InvalidShape)?;
        let role = required_string(message, "role")?;
        let allowed = match role.as_str() {
            "system" | "developer" | "user" => &["role", "content"][..],
            "assistant" => &["role", "content", "reasoning_content", "tool_calls"][..],
            "tool" => &["role", "content", "tool_call_id"][..],
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        };
        ensure_only_fields(message, allowed)?;
        if let Some(Value::Array(parts)) = message.get("content") {
            for part in parts {
                validate_chat_bridge_part(part)?;
            }
        }
        if let Some(calls) = message.get("tool_calls") {
            let calls = calls.as_array().ok_or(StaticCodecError::InvalidShape)?;
            for call in calls {
                let call = call.as_object().ok_or(StaticCodecError::InvalidShape)?;
                ensure_only_fields(call, &["id", "type", "function"])?;
                let function = call
                    .get("function")
                    .and_then(Value::as_object)
                    .ok_or(StaticCodecError::InvalidShape)?;
                ensure_only_fields(function, &["name", "arguments"])?;
            }
        }
    }
    Ok(())
}

fn validate_chat_bridge_part(value: &Value) -> Result<(), StaticCodecError> {
    let part = value.as_object().ok_or(StaticCodecError::InvalidShape)?;
    match part.get("type").and_then(Value::as_str) {
        Some("text") => ensure_only_fields(part, &["type", "text"]),
        Some("image_url") => {
            ensure_only_fields(part, &["type", "image_url"])?;
            let image = part
                .get("image_url")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            ensure_only_fields(image, &["url", "detail"])
        }
        Some("input_audio") => {
            ensure_only_fields(part, &["type", "input_audio"])?;
            let audio = part
                .get("input_audio")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            ensure_only_fields(audio, &["data", "format"])
        }
        Some("file") => {
            ensure_only_fields(part, &["type", "file"])?;
            let file = part
                .get("file")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            ensure_only_fields(file, &["file_data", "filename"])
        }
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn validate_responses_bridge_source(source: &Map<String, Value>) -> Result<(), StaticCodecError> {
    let Some(items) = source.get("input").and_then(Value::as_array) else {
        return Ok(());
    };
    for item in items {
        let item = item.as_object().ok_or(StaticCodecError::InvalidShape)?;
        match item.get("type").and_then(Value::as_str) {
            None => {
                ensure_only_fields(item, &["role", "content"])?;
                validate_responses_content(item.get("content"))
            }
            Some("message") => {
                ensure_only_fields(item, &["type", "role", "content"])?;
                validate_responses_content(item.get("content"))
            }
            Some("input_file") => ensure_only_fields(item, &["type", "file_url", "file_data"]),
            Some("input_image") => ensure_only_fields(item, &["type", "image_url", "detail"]),
            Some("reasoning") => {
                ensure_only_fields(
                    item,
                    &[
                        "type",
                        "id",
                        "status",
                        "content",
                        "summary",
                        "encrypted_content",
                    ],
                )?;
                validate_completed_status(item)?;
                validate_reasoning_bridge_parts(item.get("content"), "reasoning_text")?;
                validate_reasoning_bridge_parts(item.get("summary"), "summary_text")
            }
            Some("function_call") => {
                ensure_only_fields(item, &["type", "id", "call_id", "name", "arguments"])
            }
            Some("function_call_output") => {
                ensure_only_fields(item, &["type", "call_id", "output"])
            }
            _ => Err(StaticCodecError::UnsupportedSemantics),
        }?;
    }
    Ok(())
}

fn validate_responses_content(value: Option<&Value>) -> Result<(), StaticCodecError> {
    let Some(Value::Array(parts)) = value else {
        return Ok(());
    };
    for part in parts {
        let part = part.as_object().ok_or(StaticCodecError::InvalidShape)?;
        match part.get("type").and_then(Value::as_str) {
            Some("input_text") => ensure_only_fields(part, &["type", "text"]),
            Some("input_image") => ensure_only_fields(part, &["type", "image_url", "detail"]),
            Some("input_file") => ensure_only_fields(part, &["type", "file_url", "file_data"]),
            _ => Err(StaticCodecError::UnsupportedSemantics),
        }?;
    }
    Ok(())
}

fn validate_reasoning_bridge_parts(
    value: Option<&Value>,
    expected_type: &str,
) -> Result<(), StaticCodecError> {
    let Some(value) = value else {
        return Ok(());
    };
    let parts = value.as_array().ok_or(StaticCodecError::InvalidShape)?;
    for part in parts {
        let part = part.as_object().ok_or(StaticCodecError::InvalidShape)?;
        ensure_only_fields(part, &["type", "text"])?;
        if part.get("type").and_then(Value::as_str) != Some(expected_type) {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
    }
    Ok(())
}

fn ensure_only_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), StaticCodecError> {
    if object.keys().all(|field| allowed.contains(&field.as_str())) {
        Ok(())
    } else {
        Err(StaticCodecError::UnsupportedSemantics)
    }
}

fn validate_completed_status(object: &Map<String, Value>) -> Result<(), StaticCodecError> {
    match object.get("status") {
        None => Ok(()),
        Some(Value::String(status)) if status == "completed" => Ok(()),
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn decode_chat_input(
    source: &Map<String, Value>,
    max_bytes: usize,
) -> Result<Vec<InputItem>, StaticCodecError> {
    let messages = source
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(StaticCodecError::InvalidShape)?;
    let mut input = Vec::new();
    let mut known_calls = BTreeSet::new();
    let mut seen_results = BTreeSet::new();
    let mut item_ids = BTreeSet::new();
    for (ordinal, message) in messages.iter().enumerate() {
        let message = message.as_object().ok_or(StaticCodecError::InvalidShape)?;
        let role = required_string(message, "role")?;
        if role != "assistant"
            && message
                .get("reasoning_content")
                .is_some_and(|value| !value.is_null())
        {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        match role.as_str() {
            "system" | "developer" => {
                let text = decode_chat_instruction_content(message, max_bytes)?;
                input.push(InputItem::Instruction(Instruction::new(
                    if role == "system" {
                        InstructionAuthority::System
                    } else {
                        InstructionAuthority::Developer
                    },
                    InstructionOrigin::Downstream,
                    text,
                )));
            }
            "user" => input.push(InputItem::Message(decode_chat_message(
                MessageRole::User,
                message,
                max_bytes,
            )?)),
            "assistant" => {
                if let Some(reasoning) = optional_nonempty_string(message, "reasoning_content")? {
                    input.push(InputItem::ReasoningReplay(reasoning_item(
                        format!("reasoning_{ordinal}"),
                        reasoning,
                        max_bytes,
                    )?));
                }
                if let Some(calls) = message.get("tool_calls") {
                    if message.get("content").is_some_and(|value| {
                        !value.is_null() && value.as_str().is_none_or(|text| !text.is_empty())
                    }) {
                        return Err(StaticCodecError::UnsupportedSemantics);
                    }
                    let calls = calls.as_array().ok_or(StaticCodecError::InvalidShape)?;
                    for call in calls {
                        let call = call.as_object().ok_or(StaticCodecError::InvalidShape)?;
                        if call.get("type").and_then(Value::as_str) != Some("function") {
                            return Err(StaticCodecError::UnsupportedSemantics);
                        }
                        let function = call
                            .get("function")
                            .and_then(Value::as_object)
                            .ok_or(StaticCodecError::InvalidShape)?;
                        let call_id = required_string(call, "id")?;
                        if !known_calls.insert(call_id.clone()) {
                            return Err(StaticCodecError::InvalidToolIdentity);
                        }
                        let item_id = allocate_item_id(&call_id, ordinal + 1, &mut item_ids);
                        input.push(InputItem::PriorToolCall(tool_call(
                            item_id,
                            call_id,
                            required_string(function, "name")?,
                            required_string(function, "arguments")?,
                            max_bytes,
                        )?));
                    }
                } else if let Some(content) = message.get("content")
                    && !content.is_null()
                    && content.as_str() != Some("")
                {
                    input.push(InputItem::Message(decode_chat_message(
                        MessageRole::Assistant,
                        message,
                        max_bytes,
                    )?));
                }
            }
            "tool" => {
                let call_id = required_string(message, "tool_call_id")?;
                if !known_calls.contains(&call_id) || !seen_results.insert(call_id.clone()) {
                    return Err(StaticCodecError::InvalidToolIdentity);
                }
                let output = required_string(message, "content")?;
                input.push(InputItem::ToolResult(ToolResult::new(
                    item_id(format!("tool_result_{ordinal}"), max_bytes)?,
                    call_id_value(call_id, max_bytes)?,
                    ToolResultStatus::Success,
                    vec![ToolOutput::Text(text_value(output, max_bytes)?)],
                    None,
                )));
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    Ok(input)
}

fn decode_chat_instruction_content(
    message: &Map<String, Value>,
    max_bytes: usize,
) -> Result<TextValue, StaticCodecError> {
    match message.get("content") {
        Some(Value::String(text)) => text_value(text.clone(), max_bytes),
        Some(Value::Array(parts)) => {
            let mut text = String::new();
            for part in parts {
                let part = part.as_object().ok_or(StaticCodecError::InvalidShape)?;
                if part.get("type").and_then(Value::as_str) != Some("text") {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                text.push_str(&required_string(part, "text")?);
            }
            text_value(text, max_bytes)
        }
        _ => Err(StaticCodecError::InvalidShape),
    }
}

fn decode_chat_message(
    role: MessageRole,
    message: &Map<String, Value>,
    max_bytes: usize,
) -> Result<Message, StaticCodecError> {
    let content = message
        .get("content")
        .ok_or(StaticCodecError::InvalidShape)?;
    let parts = match content {
        Value::String(text) if !text.is_empty() => {
            vec![ContentPart::text(text_value(text.clone(), max_bytes)?)]
        }
        Value::Array(parts) => parts
            .iter()
            .map(|part| decode_chat_part(part, max_bytes))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(StaticCodecError::InvalidShape),
    };
    Message::new(role, parts).map_err(StaticCodecError::from_validation)
}

fn decode_chat_part(value: &Value, max_bytes: usize) -> Result<ContentPart, StaticCodecError> {
    let part = value.as_object().ok_or(StaticCodecError::InvalidShape)?;
    match part.get("type").and_then(Value::as_str) {
        Some("text") => Ok(ContentPart::text(text_value(
            required_string(part, "text")?,
            max_bytes,
        )?)),
        Some("image_url") => {
            let image = part
                .get("image_url")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            Ok(ContentPart::Resource(Resource::Image(ImageResource::new(
                resource_source(required_string(image, "url")?, max_bytes)?,
                None,
                image_detail(image.get("detail"))?,
            ))))
        }
        Some("input_audio") => {
            let audio = part
                .get("input_audio")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            Ok(ContentPart::Resource(Resource::Audio(AudioResource::new(
                resource_source(required_string(audio, "data")?, max_bytes)?,
                None,
            ))))
        }
        Some("file") => {
            let file = part
                .get("file")
                .and_then(Value::as_object)
                .ok_or(StaticCodecError::InvalidShape)?;
            Ok(ContentPart::Resource(Resource::File(FileResource::new(
                resource_source(required_string(file, "file_data")?, max_bytes)?,
                None,
            ))))
        }
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn decode_responses_part(value: &Value, max_bytes: usize) -> Result<ContentPart, StaticCodecError> {
    let part = value.as_object().ok_or(StaticCodecError::InvalidShape)?;
    match part.get("type").and_then(Value::as_str) {
        Some("input_text") => Ok(ContentPart::text(text_value(
            required_string(part, "text")?,
            max_bytes,
        )?)),
        Some("input_image") => Ok(ContentPart::Resource(Resource::Image(ImageResource::new(
            resource_source(required_string(part, "image_url")?, max_bytes)?,
            None,
            image_detail(part.get("detail"))?,
        )))),
        Some("input_file") => {
            let source = part
                .get("file_url")
                .or_else(|| part.get("file_data"))
                .and_then(Value::as_str)
                .ok_or(StaticCodecError::InvalidShape)?;
            Ok(ContentPart::Resource(Resource::File(FileResource::new(
                resource_source(source.to_owned(), max_bytes)?,
                None,
            ))))
        }
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn resource_source(value: String, max_bytes: usize) -> Result<ResourceSource, StaticCodecError> {
    if value.contains("://") || value.starts_with("data:") {
        return UrlValue::new(value, max_bytes)
            .map(ResourceSource::Url)
            .map_err(StaticCodecError::from_validation);
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&value)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&value))
        .map_err(|_| StaticCodecError::InvalidShape)?;
    let bytes = BoundedBytes::new(bytes, max_bytes).map_err(StaticCodecError::from_validation)?;
    InlineResource::new(bytes)
        .map(ResourceSource::Inline)
        .map_err(StaticCodecError::from_validation)
}

fn image_detail(value: Option<&Value>) -> Result<Option<ImageDetail>, StaticCodecError> {
    value
        .filter(|value| !value.is_null())
        .map(|value| match value.as_str() {
            Some("auto") => Ok(ImageDetail::Auto),
            Some("low") => Ok(ImageDetail::Low),
            Some("high") => Ok(ImageDetail::High),
            _ => Err(StaticCodecError::UnsupportedSemantics),
        })
        .transpose()
}

fn decode_responses_input(
    source: &Map<String, Value>,
    max_bytes: usize,
) -> Result<Vec<InputItem>, StaticCodecError> {
    let mut input = Vec::new();
    if let Some(instructions) = source.get("instructions") {
        let instructions = instructions
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or(StaticCodecError::InvalidShape)?;
        input.push(InputItem::Instruction(Instruction::new(
            InstructionAuthority::System,
            InstructionOrigin::Downstream,
            text_value(instructions.to_owned(), max_bytes)?,
        )));
    }
    let wire_input = source.get("input").ok_or(StaticCodecError::InvalidShape)?;
    if let Some(text) = wire_input.as_str() {
        input.push(InputItem::Message(
            Message::new(
                MessageRole::User,
                vec![ContentPart::text(text_value(text.to_owned(), max_bytes)?)],
            )
            .map_err(StaticCodecError::from_validation)?,
        ));
        return Ok(input);
    }
    let items = wire_input
        .as_array()
        .ok_or(StaticCodecError::InvalidShape)?;
    let mut known_calls = BTreeSet::new();
    let mut seen_results = BTreeSet::new();
    let mut item_ids = BTreeSet::new();
    for (ordinal, item) in items.iter().enumerate() {
        let item = item.as_object().ok_or(StaticCodecError::InvalidShape)?;
        let kind = item.get("type").and_then(Value::as_str);
        let shorthand = kind.is_none()
            && item.len() == 2
            && item.contains_key("role")
            && item.contains_key("content");
        match kind {
            Some("message") if !shorthand => {
                input.push(decode_responses_message(item, max_bytes)?);
            }
            None if shorthand => input.push(decode_responses_message(item, max_bytes)?),
            Some("input_file") | Some("input_image") => {
                let part = decode_responses_part(&Value::Object(item.clone()), max_bytes)?;
                input.push(InputItem::Message(
                    Message::new(MessageRole::User, vec![part])
                        .map_err(StaticCodecError::from_validation)?,
                ));
            }
            Some("reasoning") => {
                let id = required_string(item, "id")?;
                let mut parts = Vec::new();
                decode_reasoning_parts(item, "content", "reasoning_text", max_bytes, &mut parts)?;
                decode_reasoning_parts(item, "summary", "summary_text", max_bytes, &mut parts)?;
                if item.get("encrypted_content").is_some_and(|value| {
                    !value.is_null() && value.as_str().is_none_or(|text| !text.is_empty())
                }) {
                    return Err(StaticCodecError::UnsupportedSemantics);
                }
                let reasoning = ReasoningItem::new(item_id(id, max_bytes)?, parts, None)
                    .map_err(|_| StaticCodecError::InvalidShape)?;
                input.push(InputItem::ReasoningReplay(reasoning));
            }
            Some("function_call") => {
                let call_id = required_string(item, "call_id")?;
                if !known_calls.insert(call_id.clone()) {
                    return Err(StaticCodecError::InvalidToolIdentity);
                }
                let id = required_string(item, "id")?;
                if !item_ids.insert(id.clone()) {
                    return Err(StaticCodecError::InvalidToolIdentity);
                }
                input.push(InputItem::PriorToolCall(tool_call(
                    id,
                    call_id,
                    required_string(item, "name")?,
                    required_string(item, "arguments")?,
                    max_bytes,
                )?));
            }
            Some("function_call_output") => {
                let call_id = required_string(item, "call_id")?;
                if !known_calls.contains(&call_id) || !seen_results.insert(call_id.clone()) {
                    return Err(StaticCodecError::InvalidToolIdentity);
                }
                let output = item.get("output").ok_or(StaticCodecError::InvalidShape)?;
                let output = output
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| output.to_string());
                input.push(InputItem::ToolResult(ToolResult::new(
                    item_id(format!("tool_result_{ordinal}"), max_bytes)?,
                    call_id_value(call_id, max_bytes)?,
                    ToolResultStatus::Success,
                    vec![ToolOutput::Text(text_value(output, max_bytes)?)],
                    None,
                )));
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    Ok(input)
}

fn decode_responses_message(
    item: &Map<String, Value>,
    max_bytes: usize,
) -> Result<InputItem, StaticCodecError> {
    let role = required_string(item, "role")?;
    let content = item.get("content").ok_or(StaticCodecError::InvalidShape)?;
    let parts = match content {
        Value::String(text) => vec![ContentPart::text(text_value(text.clone(), max_bytes)?)],
        Value::Array(parts) => parts
            .iter()
            .map(|part| decode_responses_part(part, max_bytes))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(StaticCodecError::InvalidShape),
    };
    match role.as_str() {
        "system" | "developer" => {
            let text = flatten_text(&parts)?;
            Ok(InputItem::Instruction(Instruction::new(
                if role == "system" {
                    InstructionAuthority::System
                } else {
                    InstructionAuthority::Developer
                },
                InstructionOrigin::Downstream,
                text_value(text, max_bytes)?,
            )))
        }
        "user" | "assistant" => Ok(InputItem::Message(
            Message::new(
                if role == "user" {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                parts,
            )
            .map_err(StaticCodecError::from_validation)?,
        )),
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
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
        let text = text_value(required_string(part, "text")?, max_bytes)?;
        target.push(if field == "content" {
            ReasoningPart::Visible(text)
        } else {
            ReasoningPart::Summary(text)
        });
    }
    Ok(())
}

fn decode_tools(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
    max_bytes: usize,
) -> Result<Vec<ToolDefinition>, StaticCodecError> {
    let Some(tools) = source.get("tools") else {
        return Ok(Vec::new());
    };
    let tools = tools.as_array().ok_or(StaticCodecError::InvalidShape)?;
    tools
        .iter()
        .map(|tool| {
            let tool = tool.as_object().ok_or(StaticCodecError::InvalidShape)?;
            if tool.get("type").and_then(Value::as_str) != Some("function") {
                return Err(StaticCodecError::UnsupportedSemantics);
            }
            let function = match protocol {
                ApiProtocol::ChatCompletions => tool
                    .get("function")
                    .and_then(Value::as_object)
                    .ok_or(StaticCodecError::InvalidShape)?,
                ApiProtocol::Responses => tool,
            };
            let name = tool_name(required_string(function, "name")?, max_bytes)?;
            let description = function
                .get("description")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or(StaticCodecError::InvalidShape)
                        .and_then(|value| text_value(value.to_owned(), max_bytes))
                })
                .transpose()?;
            let parameters = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"}));
            let schema = JsonSchema::new(parameters, max_bytes)
                .map_err(StaticCodecError::from_validation)?;
            let strict = optional_bool(function, "strict")?.unwrap_or(false);
            Ok(ToolDefinition::new(
                name,
                ToolOrigin::Downstream,
                ToolExecutor::Client,
                ToolVisibility::Public,
                ToolKind::Function(FunctionTool::new(description, schema, strict)),
            ))
        })
        .collect()
}

fn decode_tool_choice(
    protocol: ApiProtocol,
    choice: Option<&Value>,
    tools: &[ToolDefinition],
    max_bytes: usize,
) -> Result<ToolChoice, StaticCodecError> {
    let Some(choice) = choice else {
        return Ok(if tools.is_empty() {
            ToolChoice::None
        } else {
            ToolChoice::Auto
        });
    };
    if let Some(choice) = choice.as_str() {
        return match choice {
            "none" => Ok(ToolChoice::None),
            "auto" => Ok(ToolChoice::Auto),
            "required" => Ok(ToolChoice::Required),
            _ => Err(StaticCodecError::UnsupportedSemantics),
        };
    }
    let choice = choice.as_object().ok_or(StaticCodecError::InvalidShape)?;
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let name = match protocol {
        ApiProtocol::ChatCompletions => choice
            .get("function")
            .and_then(Value::as_object)
            .ok_or(StaticCodecError::InvalidShape)
            .and_then(|function| required_string(function, "name"))?,
        ApiProtocol::Responses => required_string(choice, "name")?,
    };
    Ok(ToolChoice::Specific(tool_name(name, max_bytes)?))
}

fn decode_parallel(
    value: Option<&Value>,
    no_tools: bool,
) -> Result<ParallelToolCalls, StaticCodecError> {
    let Some(value) = value else {
        return Ok(ParallelToolCalls::Inactive);
    };
    if no_tools {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    match value.as_bool() {
        Some(true) => Ok(ParallelToolCalls::Allow),
        Some(false) => Ok(ParallelToolCalls::RequireSerial),
        None => Err(StaticCodecError::InvalidShape),
    }
}

fn decode_controls(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
    parallel: ParallelToolCalls,
) -> Result<GenerationControls, StaticCodecError> {
    let max_output_tokens = match protocol {
        ApiProtocol::ChatCompletions => source
            .get("max_completion_tokens")
            .or_else(|| source.get("max_tokens")),
        ApiProtocol::Responses => source.get("max_output_tokens"),
    }
    .map(parse_u64)
    .transpose()?;
    let temperature = source.get("temperature").map(parse_f64).transpose()?;
    let top_p = source.get("top_p").map(parse_f64).transpose()?;
    GenerationControls::new(max_output_tokens, None)
        .and_then(|controls| controls.with_sampling(temperature, top_p, None))
        .map(|controls| controls.with_parallel_tool_calls(parallel))
        .map_err(|_| StaticCodecError::InvalidShape)
}

fn decode_output(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
    max_bytes: usize,
) -> Result<OutputConstraint, StaticCodecError> {
    let format = match protocol {
        ApiProtocol::ChatCompletions => source.get("response_format"),
        ApiProtocol::Responses => match source.get("text").filter(|value| !value.is_null()) {
            None => None,
            Some(text) => {
                let text = text.as_object().ok_or(StaticCodecError::InvalidShape)?;
                reject_unknown_keys(text, &["format"])?;
                text.get("format")
            }
        },
    };
    let Some(format) = format.filter(|value| !value.is_null()) else {
        return Ok(OutputConstraint::Text);
    };
    let format = format.as_object().ok_or(StaticCodecError::InvalidShape)?;
    match protocol {
        ApiProtocol::ChatCompletions => {
            reject_unknown_keys(format, &["type", "json_schema"])?;
        }
        ApiProtocol::Responses => {
            reject_unknown_keys(format, &["type", "name", "description", "schema", "strict"])?;
        }
    }
    match format.get("type").and_then(Value::as_str) {
        Some("text") => Ok(OutputConstraint::Text),
        Some("json_object") => Ok(OutputConstraint::JsonObject),
        Some("json_schema") => {
            let schema = match protocol {
                ApiProtocol::ChatCompletions => format
                    .get("json_schema")
                    .and_then(Value::as_object)
                    .ok_or(StaticCodecError::InvalidShape)?,
                ApiProtocol::Responses => format,
            };
            if protocol == ApiProtocol::ChatCompletions {
                reject_unknown_keys(schema, &["name", "description", "schema", "strict"])?;
            }
            let name = text_value(required_string(schema, "name")?, max_bytes)?;
            let description = schema
                .get("description")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or(StaticCodecError::InvalidShape)
                        .and_then(|value| text_value(value.to_owned(), max_bytes))
                })
                .transpose()?;
            let value = schema
                .get("schema")
                .cloned()
                .ok_or(StaticCodecError::InvalidShape)?;
            let schema_value =
                JsonSchema::new(value, max_bytes).map_err(StaticCodecError::from_validation)?;
            let strict = optional_bool(schema, "strict")?.unwrap_or(false);
            Ok(OutputConstraint::JsonSchema {
                name,
                description,
                schema: schema_value,
                strict,
            })
        }
        _ => Err(StaticCodecError::UnsupportedSemantics),
    }
}

fn decode_reasoning(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<ReasoningRequest, StaticCodecError> {
    match protocol {
        ApiProtocol::ChatCompletions => {
            let Some(value) = source.get("reasoning_effort") else {
                return Ok(ReasoningRequest::default());
            };
            let effort = value.as_str().ok_or(StaticCodecError::InvalidShape)?;
            let effort =
                ReasoningEffort::from_wire(effort).ok_or(StaticCodecError::UnsupportedSemantics)?;
            Ok(ReasoningRequest::new(effort, ReasoningSummary::Omitted))
        }
        ApiProtocol::Responses => {
            let Some(reasoning) = source.get("reasoning") else {
                return Ok(ReasoningRequest::default());
            };
            let reasoning = reasoning
                .as_object()
                .ok_or(StaticCodecError::InvalidShape)?;
            reject_unknown_keys(reasoning, &["effort", "summary"])?;
            let effort = reasoning
                .get("effort")
                .map(|value| {
                    value
                        .as_str()
                        .and_then(ReasoningEffort::from_wire)
                        .ok_or(StaticCodecError::UnsupportedSemantics)
                })
                .transpose()?
                .unwrap_or(ReasoningEffort::Omitted);
            let summary = reasoning
                .get("summary")
                .map(|value| match value {
                    Value::Bool(false) => Ok(ReasoningSummary::Disabled),
                    Value::String(value) if value == "auto" => Ok(ReasoningSummary::Auto),
                    _ => Err(StaticCodecError::UnsupportedSemantics),
                })
                .transpose()?
                .unwrap_or(ReasoningSummary::Omitted);
            Ok(ReasoningRequest::new(effort, summary))
        }
    }
}

fn decode_state(
    source: &Map<String, Value>,
    max_bytes: usize,
) -> Result<RequestState, StaticCodecError> {
    let cache = source
        .get("prompt_cache_key")
        .map(|value| {
            let key = value.as_str().ok_or(StaticCodecError::InvalidShape)?;
            let key = CacheKey::new(key.to_owned(), max_bytes)
                .map_err(|_| StaticCodecError::InvalidShape)?;
            Ok::<_, StaticCodecError>(CacheDirective::new(Some(key), None))
        })
        .transpose()?;
    Ok(RequestState::new(None, cache, false))
}

fn encode_chat_input(input: &[InputItem]) -> Result<Vec<Value>, StaticCodecError> {
    let mut messages = Vec::new();
    let mut pending_calls = Vec::new();
    let mut pending_reasoning = String::new();
    for item in input {
        match item {
            InputItem::Instruction(instruction) => {
                flush_calls(&mut messages, &mut pending_calls, &mut pending_reasoning);
                messages.push(json!({
                    "content": instruction.text().as_str(),
                    "role": match instruction.authority() {
                        InstructionAuthority::System => "system",
                        InstructionAuthority::Developer => "developer",
                    }
                }));
            }
            InputItem::Message(message) => {
                flush_calls(&mut messages, &mut pending_calls, &mut pending_reasoning);
                let mut value = json!({
                    "content": flatten_text(message.content())?,
                    "role": match message.role() {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                    }
                });
                if !pending_reasoning.is_empty() && message.role() == MessageRole::Assistant {
                    value["reasoning_content"] =
                        Value::String(std::mem::take(&mut pending_reasoning));
                }
                messages.push(value);
            }
            InputItem::ReasoningReplay(reasoning) => {
                for part in reasoning.parts() {
                    match part {
                        ReasoningPart::Visible(text) | ReasoningPart::Summary(text) => {
                            pending_reasoning.push_str(text.as_str());
                        }
                        ReasoningPart::Opaque(_) => {
                            return Err(StaticCodecError::UnsupportedSemantics);
                        }
                    }
                }
            }
            InputItem::PriorToolCall(call) => {
                pending_calls.push(json!({
                    "function": {
                        "arguments": Value::Object(call.input().as_function()?.as_map().clone()).to_string(),
                        "name": call.tool().as_str()
                    },
                    "id": call.call_id().as_str(),
                    "type": "function"
                }));
            }
            InputItem::ToolResult(result) => {
                flush_calls(&mut messages, &mut pending_calls, &mut pending_reasoning);
                messages.push(json!({
                    "content": flatten_tool_output(result.output())?,
                    "role": "tool",
                    "tool_call_id": result.call_id().as_str()
                }));
            }
            InputItem::Extension(_) => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    flush_calls(&mut messages, &mut pending_calls, &mut pending_reasoning);
    if !pending_reasoning.is_empty() {
        messages.push(json!({
            "content": Value::Null,
            "reasoning_content": pending_reasoning,
            "role": "assistant"
        }));
    }
    Ok(messages)
}

fn flush_calls(
    messages: &mut Vec<Value>,
    pending_calls: &mut Vec<Value>,
    pending_reasoning: &mut String,
) {
    if pending_calls.is_empty() {
        return;
    }
    let mut message = json!({
        "content": Value::Null,
        "role": "assistant",
        "tool_calls": std::mem::take(pending_calls),
    });
    if !pending_reasoning.is_empty() {
        message["reasoning_content"] = Value::String(std::mem::take(pending_reasoning));
    }
    messages.push(message);
}

fn encode_responses_input(
    input: &[InputItem],
    no_tools: bool,
    stream: bool,
) -> Result<(Option<String>, Value), StaticCodecError> {
    let mut items = Vec::new();
    let mut instructions = None;
    for (ordinal, item) in input.iter().enumerate() {
        match item {
            InputItem::Instruction(instruction) if ordinal == 0 && instructions.is_none() => {
                instructions = Some(instruction.text().as_str().to_owned());
            }
            InputItem::Instruction(instruction) => items.push(json!({
                "content": [{"text": instruction.text().as_str(), "type": "input_text"}],
                "role": match instruction.authority() {
                    InstructionAuthority::System => "system",
                    InstructionAuthority::Developer => "developer",
                },
                "type": "message"
            })),
            InputItem::Message(message) => {
                let text = flatten_text(message.content())?;
                let content = if no_tools {
                    json!([{"text": text, "type": "input_text"}])
                } else {
                    Value::String(text)
                };
                items.push(json!({
                    "content": content,
                    "role": match message.role() {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                    },
                    "type": "message"
                }));
            }
            InputItem::ReasoningReplay(reasoning) => {
                let mut content = Vec::new();
                let mut summary = Vec::new();
                for part in reasoning.parts() {
                    match part {
                        ReasoningPart::Visible(text) => content.push(json!({
                            "text": text.as_str(), "type": "reasoning_text"
                        })),
                        ReasoningPart::Summary(text) => summary.push(json!({
                            "text": text.as_str(), "type": "summary_text"
                        })),
                        ReasoningPart::Opaque(_) => {
                            return Err(StaticCodecError::UnsupportedSemantics);
                        }
                    }
                }
                items.push(json!({
                    "content": content,
                    "id": reasoning.id().as_str(),
                    "status": "completed",
                    "summary": summary,
                    "type": "reasoning"
                }));
            }
            InputItem::PriorToolCall(call) => items.push(json!({
                "arguments": Value::Object(call.input().as_function()?.as_map().clone()).to_string(),
                "call_id": call.call_id().as_str(),
                "id": call.id().as_str(),
                "name": call.tool().as_str(),
                "type": "function_call"
            })),
            InputItem::ToolResult(result) => items.push(json!({
                "call_id": result.call_id().as_str(),
                "output": flatten_tool_output(result.output())?,
                "type": "function_call_output"
            })),
            InputItem::Extension(_) => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    if stream && instructions.is_none() && items.len() == 1 {
        let text = match items[0].get("content") {
            Some(Value::String(text)) => Some(text.as_str()),
            Some(Value::Array(parts)) if parts.len() == 1 => parts.first().and_then(|part| {
                (part.get("type").and_then(Value::as_str) == Some("input_text"))
                    .then(|| part.get("text").and_then(Value::as_str))
                    .flatten()
            }),
            _ => None,
        };
        if let Some(text) = text {
            return Ok((None, Value::String(text.to_owned())));
        }
    }
    Ok((instructions, Value::Array(items)))
}

fn encode_tools(
    protocol: ApiProtocol,
    request: &GenerationRequest,
    provider_profile: Option<&ProviderToolProfile>,
    target: &mut Map<String, Value>,
) -> Result<(), StaticCodecError> {
    if !request.tools().is_empty() {
        let tools = request
            .tools()
            .iter()
            .map(|tool| {
                let ToolKind::Function(function) = tool.kind() else {
                    if protocol != ApiProtocol::Responses {
                        return Err(StaticCodecError::UnsupportedSemantics);
                    }
                    let profile = provider_profile.ok_or(StaticCodecError::UnsupportedSemantics)?;
                    return match lower_provider_server_tool(tool, profile)
                        .map_err(|_| StaticCodecError::UnsupportedSemantics)?
                    {
                        ProviderServerTool::WebSearch => Ok(json!({"type": "web_search"})),
                    };
                };
                let mut definition = Map::new();
                definition.insert(
                    "name".to_owned(),
                    Value::String(tool.name().as_str().to_owned()),
                );
                if let Some(description) = function.description() {
                    definition.insert(
                        "description".to_owned(),
                        Value::String(description.as_str().to_owned()),
                    );
                }
                definition.insert(
                    "parameters".to_owned(),
                    function.parameters().as_value().clone(),
                );
                if function.strict() {
                    definition.insert("strict".to_owned(), Value::Bool(true));
                }
                Ok(match protocol {
                    ApiProtocol::ChatCompletions => json!({
                        "function": Value::Object(definition),
                        "type": "function"
                    }),
                    ApiProtocol::Responses => {
                        definition.insert("type".to_owned(), Value::String("function".to_owned()));
                        Value::Object(definition)
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        target.insert("tools".to_owned(), Value::Array(tools));
    }
    match request.tool_choice() {
        ToolChoice::None if request.tools().is_empty() => {}
        ToolChoice::None => {
            target.insert("tool_choice".to_owned(), Value::String("none".to_owned()));
        }
        ToolChoice::Auto => {}
        ToolChoice::Required => {
            target.insert(
                "tool_choice".to_owned(),
                Value::String("required".to_owned()),
            );
        }
        ToolChoice::Specific(name) => {
            target.insert(
                "tool_choice".to_owned(),
                match protocol {
                    ApiProtocol::ChatCompletions => json!({
                        "function": {"name": name.as_str()}, "type": "function"
                    }),
                    ApiProtocol::Responses => {
                        json!({"name": name.as_str(), "type": "function"})
                    }
                },
            );
        }
    }
    match request.parallel_tool_calls() {
        ParallelToolCalls::Inactive => {}
        ParallelToolCalls::Allow => {
            target.insert("parallel_tool_calls".to_owned(), Value::Bool(true));
        }
        ParallelToolCalls::RequireSerial => {
            target.insert("parallel_tool_calls".to_owned(), Value::Bool(false));
        }
    }
    Ok(())
}

fn encode_controls(
    protocol: ApiProtocol,
    controls: &GenerationControls,
    target: &mut Map<String, Value>,
) {
    if let Some(max_tokens) = controls.max_output_tokens() {
        target.insert(
            match protocol {
                ApiProtocol::ChatCompletions => "max_completion_tokens",
                ApiProtocol::Responses => "max_output_tokens",
            }
            .to_owned(),
            Value::from(max_tokens),
        );
    }
    if let Some(temperature) = controls.temperature() {
        target.insert("temperature".to_owned(), json!(temperature));
    }
    if let Some(top_p) = controls.top_p() {
        target.insert("top_p".to_owned(), json!(top_p));
    }
}

fn encode_output(
    protocol: ApiProtocol,
    output: &OutputConstraint,
    target: &mut Map<String, Value>,
) {
    let format = match output {
        OutputConstraint::Text => return,
        OutputConstraint::JsonObject => json!({"type": "json_object"}),
        OutputConstraint::JsonSchema {
            name,
            description,
            schema,
            strict,
        } => {
            let mut schema_format = json!({
                "name": name.as_str(),
                "schema": schema.as_value(),
                "strict": strict
            });
            if let Some(description) = description {
                schema_format["description"] = Value::String(description.as_str().to_owned());
            }
            match protocol {
                ApiProtocol::ChatCompletions => json!({
                    "json_schema": schema_format,
                    "type": "json_schema"
                }),
                ApiProtocol::Responses => {
                    schema_format["type"] = Value::String("json_schema".to_owned());
                    schema_format
                }
            }
        }
    };
    match protocol {
        ApiProtocol::ChatCompletions => {
            target.insert("response_format".to_owned(), format);
        }
        ApiProtocol::Responses => {
            target.insert("text".to_owned(), json!({"format": format}));
        }
    }
}

fn encode_reasoning(
    protocol: ApiProtocol,
    reasoning: ReasoningRequest,
    force_summary: bool,
    target: &mut Map<String, Value>,
) -> Result<(), StaticCodecError> {
    let Some(effort) = reasoning.effort().as_wire() else {
        return Ok(());
    };
    match protocol {
        ApiProtocol::ChatCompletions => {
            target.insert(
                "reasoning_effort".to_owned(),
                Value::String(effort.to_owned()),
            );
        }
        ApiProtocol::Responses => {
            let mut value = json!({"effort": effort});
            let summary = if force_summary && effort != "none" {
                Some("auto")
            } else {
                match reasoning.summary() {
                    ReasoningSummary::Auto => Some("auto"),
                    ReasoningSummary::Disabled => None,
                    ReasoningSummary::Omitted => None,
                }
            };
            if let Some(summary) = summary {
                value["summary"] = Value::String(summary.to_owned());
            }
            target.insert("reasoning".to_owned(), value);
        }
    }
    Ok(())
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

fn flatten_tool_output(output: &[ToolOutput]) -> Result<String, StaticCodecError> {
    let mut text = String::new();
    for value in output {
        match value {
            ToolOutput::Text(value) => text.push_str(value.as_str()),
            ToolOutput::Json(value) => {
                text.push_str(&Value::Object(value.as_map().clone()).to_string())
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        }
    }
    Ok(text)
}

trait FunctionInput {
    fn as_function(&self) -> Result<&JsonObject, StaticCodecError>;
}

impl FunctionInput for ToolInput {
    fn as_function(&self) -> Result<&JsonObject, StaticCodecError> {
        match self {
            ToolInput::Function(value) => Ok(value),
            ToolInput::Server(_) | ToolInput::Extension(_) => {
                Err(StaticCodecError::UnsupportedSemantics)
            }
        }
    }
}

fn tool_call(
    item_id_value: String,
    call_id_value_string: String,
    name: String,
    arguments: String,
    max_bytes: usize,
) -> Result<ToolCall, StaticCodecError> {
    let arguments: Value =
        serde_json::from_str(&arguments).map_err(|_| StaticCodecError::InvalidToolArguments)?;
    let arguments = JsonObject::new(arguments, max_bytes)
        .map_err(|_| StaticCodecError::InvalidToolArguments)?;
    Ok(ToolCall::new(
        item_id(item_id_value, max_bytes)?,
        call_id_value(call_id_value_string, max_bytes)?,
        tool_name(name, max_bytes)?,
        ToolInput::Function(arguments),
        None,
    ))
}

fn reasoning_item(
    id: String,
    text: String,
    max_bytes: usize,
) -> Result<ReasoningItem, StaticCodecError> {
    ReasoningItem::new(
        item_id(id, max_bytes)?,
        vec![ReasoningPart::Visible(text_value(text, max_bytes)?)],
        None,
    )
    .map_err(|_| StaticCodecError::InvalidShape)
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

fn optional_bool(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<bool>, StaticCodecError> {
    object
        .get(field)
        .map(|value| value.as_bool().ok_or(StaticCodecError::InvalidShape))
        .transpose()
}

fn reject_unknown_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), StaticCodecError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    Ok(())
}

fn parse_u64(value: &Value) -> Result<u64, StaticCodecError> {
    value.as_u64().ok_or(StaticCodecError::InvalidShape)
}

fn parse_f64(value: &Value) -> Result<f64, StaticCodecError> {
    value.as_f64().ok_or(StaticCodecError::InvalidShape)
}

fn text_value(value: String, max_bytes: usize) -> Result<TextValue, StaticCodecError> {
    TextValue::new(value, max_bytes).map_err(StaticCodecError::from_validation)
}

fn tool_name(value: String, max_bytes: usize) -> Result<ToolName, StaticCodecError> {
    ToolName::new(value, max_bytes).map_err(StaticCodecError::from_validation)
}

fn item_id(value: String, max_bytes: usize) -> Result<ItemId, StaticCodecError> {
    ItemId::new(value, max_bytes).map_err(|_| StaticCodecError::InvalidShape)
}

fn call_id_value(
    value: String,
    max_bytes: usize,
) -> Result<crate::ir::generation::CallId, StaticCodecError> {
    crate::ir::generation::CallId::new(value, max_bytes).map_err(|_| StaticCodecError::InvalidShape)
}
