//! Chat Completions and Responses request analysis.
//!
//! The analyzer extracts a fixed set of generation requirements and fails closed for protocol
//! positions that are modeled but not implemented. It never selects or reorders Routes.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    core::{ApiProtocol, StructuredOutputMode, ToolChoiceMode},
    registry::ReasoningLevel,
};

use super::super::{
    error::RequestPlanningError,
    types::{RequestRequirements, RequestedCapabilities, RequestedReasoning},
};

mod audio;
mod image_input;

use audio::analyze_audio;
use image_input::analyze_image_input;

/// Parses a downstream request and extracts registry-independent request facts.
///
/// This stage does not select a Route or rewrite the request body.
pub fn analyze_request(
    protocol: ApiProtocol,
    body: &Bytes,
) -> Result<RequestRequirements, RequestPlanningError> {
    // Parse the JSON object and extract the Public Model and stream flag.
    let document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object()
        .ok_or(RequestPlanningError::InvalidJson)?;
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .ok_or(RequestPlanningError::MissingModel)?;

    // Block unimplemented protocol-specific fields before Route planning can enter Native or Bridge paths.
    reject_reserved_request_fields(protocol, object)?;

    let is_streaming = object.get("stream").and_then(Value::as_bool) == Some(true);
    // Derive the capabilities actually requested from protocol fields.
    let requested_output_tokens = requested_output_tokens(object);
    let (audio_input, voice_conditioning, audio_output) = analyze_audio(protocol, object)?;
    let asr_options = object.get("asr_options").filter(|value| !value.is_null());
    let asr_options_present = asr_options.is_some();
    let asr_language = match asr_options {
        None => None,
        Some(Value::Object(options)) => {
            match options.get("language").filter(|value| !value.is_null()) {
                None => None,
                Some(Value::String(language)) => Some(language.to_owned()),
                Some(_) => return Err(RequestPlanningError::InvalidMultimodalInput),
            }
        }
        Some(_) => return Err(RequestPlanningError::InvalidMultimodalInput),
    };
    let requests_function_calling = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(is_function_tool));
    let (function_tool_choice, unknown_tool_choice) =
        requested_tool_choice(object, requests_function_calling);
    let requests_unmodeled_tools =
        object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| {
                tools
                    .iter()
                    .any(|tool| !is_function_tool(tool) && !is_reserved_tool(protocol, tool))
            });
    let (structured_output_mode, structured_output_strict_schema, unknown_structured_output) =
        requested_structured_output(object);
    let function_tool_strict_schema = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(tool_requests_strict_mode));
    let requested_capabilities = RequestedCapabilities {
        streaming: is_streaming,
        function_tool_choice,
        unknown_tool_choice,
        function_tool_strict_schema,
        parallel_tool_calls: requests_function_calling
            && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
        image_input: analyze_image_input(protocol, object)?,
        audio_input,
        voice_conditioning,
        audio_output,
        asr_options_present,
        asr_language,
        structured_output_mode,
        structured_output_strict_schema,
        unknown_structured_output,
        store: object.get("store").and_then(Value::as_bool) == Some(true),
        unmodeled_tools: requests_unmodeled_tools,
        reasoning: requested_reasoning(protocol, object),
        previous_response_id: protocol == ApiProtocol::Responses
            && object
                .get("previous_response_id")
                .is_some_and(|value| !value.is_null()),
        background: protocol == ApiProtocol::Responses
            && object.get("background").and_then(Value::as_bool) == Some(true),
    };
    // Freeze request facts so later Route planning does not reinterpret the body.
    Ok(RequestRequirements {
        public_model: public_model.to_owned(),
        protocol,
        is_streaming,
        requested_output_tokens,
        requested_capabilities,
    })
}

/// Returns a stable planning error when reserved Chat/Responses fields are used, preventing Native passthrough.
fn reject_reserved_request_fields(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<(), RequestPlanningError> {
    // Use protocol-specific field sets so same-named or differently named Chat and Responses fields cannot mix.
    match protocol {
        ApiProtocol::ChatCompletions if requests_reserved_chat_capability(object) => {
            Err(RequestPlanningError::UnimplementedCapabilities)
        }
        ApiProtocol::Responses if requests_reserved_responses_capability(object) => {
            Err(RequestPlanningError::UnimplementedCapabilities)
        }
        ApiProtocol::ChatCompletions | ApiProtocol::Responses => Ok(()),
    }
}

/// Returns whether a Chat Completions request uses a capability reserved only in the definition.
fn requests_reserved_chat_capability(object: &serde_json::Map<String, Value>) -> bool {
    requests_reserved_tool(ApiProtocol::ChatCompletions, object)
        || chat_messages_contain_part_type(object, "file")
        || has_non_null_field(object, "prediction")
        || has_non_null_field(object, "web_search_options")
        || requests_prompt_caching(object)
        || has_non_null_field(object, "moderation")
}

/// Returns whether a Responses request uses a capability reserved only in the definition.
fn requests_reserved_responses_capability(object: &serde_json::Map<String, Value>) -> bool {
    requests_reserved_tool(ApiProtocol::Responses, object)
        || responses_input_contains_part_type(object, "input_audio")
        || responses_input_contains_part_type(object, "input_file")
        || array_field_contains(object, "modalities", "audio")
        || has_non_null_field(object, "audio")
        || has_non_null_field(object, "conversation")
        || has_non_null_field(object, "prompt")
        || requests_prompt_caching(object)
        || has_non_null_field(object, "context_management")
        || has_non_null_field(object, "include")
        || has_non_null_field(object, "moderation")
}

/// Returns whether a request contains a tool named by the current protocol capability but not implemented.
fn requests_reserved_tool(protocol: ApiProtocol, object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(|tool| is_reserved_tool(protocol, tool)))
}

/// Returns whether one tool is a protocol-known custom or Responses-hosted reserved type.
fn is_reserved_tool(protocol: ApiProtocol, tool: &Value) -> bool {
    let Some(tool_type) = tool.get("type").and_then(Value::as_str) else {
        return false;
    };
    tool_type == "custom"
        || protocol == ApiProtocol::Responses && is_responses_hosted_tool_type(tool_type)
}

/// Returns whether a wire tool type maps to the current `HostedToolKind` reserved enum.
fn is_responses_hosted_tool_type(tool_type: &str) -> bool {
    matches!(
        tool_type,
        "web_search"
            | "web_search_preview"
            | "file_search"
            | "code_interpreter"
            | "computer_use"
            | "computer_use_preview"
            | "image_generation"
            | "mcp"
            | "shell"
            | "local_shell"
            | "apply_patch"
            | "tool_search"
            | "skills"
            | "programmatic_tool_calling"
    )
}

/// Returns whether Chat message content contains the requested protocol part type.
fn chat_messages_contain_part_type(
    object: &serde_json::Map<String, Value>,
    expected_type: &str,
) -> bool {
    object
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages
                .iter()
                .any(|message| content_contains_part_type(message.get("content"), expected_type))
        })
}

/// Returns whether a Responses input item or its content contains the requested protocol part type.
fn responses_input_contains_part_type(
    object: &serde_json::Map<String, Value>,
    expected_type: &str,
) -> bool {
    object
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some(expected_type)
                    || content_contains_part_type(item.get("content"), expected_type)
            })
        })
}

/// Returns whether an array field contains the requested string.
fn array_field_contains(
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) -> bool {
    object
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(expected)))
}

/// Returns whether a field carries a non-`null` value.
fn has_non_null_field(object: &serde_json::Map<String, Value>, field: &str) -> bool {
    object.get(field).is_some_and(|value| !value.is_null())
}

/// Returns whether a request uses prompt-cache key/options/retention or a content breakpoint.
fn requests_prompt_caching(object: &serde_json::Map<String, Value>) -> bool {
    [
        "prompt_cache_key",
        "prompt_cache_options",
        "prompt_cache_retention",
    ]
    .iter()
    .any(|field| has_non_null_field(object, field))
        || object.values().any(contains_prompt_cache_breakpoint)
}

/// Recursively identifies a `prompt_cache_breakpoint` part in a content tree.
fn contains_prompt_cache_breakpoint(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(contains_prompt_cache_breakpoint),
        Value::Object(object) => {
            object.get("type").and_then(Value::as_str) == Some("prompt_cache_breakpoint")
                || object.values().any(contains_prompt_cache_breakpoint)
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

/// Returns whether a content array contains the requested protocol part type.
fn content_contains_part_type(content: Option<&Value>, expected_type: &str) -> bool {
    content.and_then(Value::as_array).is_some_and(|parts| {
        parts
            .iter()
            .any(|part| part.get("type").and_then(Value::as_str) == Some(expected_type))
    })
}

/// Function-tool detection covers only OpenAI JSON Schema function tools. Built-in and custom tools need
/// their own configuration semantics and probes; until modeled, native `tools[]` passthrough must
/// not imply support.
fn is_function_tool(tool: &Value) -> bool {
    tool.get("type").and_then(Value::as_str) == Some("function")
}

/// The Chat compatibility surface has two legacy/current output-limit fields. When clients provide
/// multiple fields, use the largest value for local ceiling validation without silently rewriting
/// the upstream request. Non-negative-integer validation remains the upstream protocol's responsibility.
fn requested_output_tokens(object: &serde_json::Map<String, Value>) -> Option<u64> {
    ["max_output_tokens", "max_completion_tokens", "max_tokens"]
        .iter()
        .filter_map(|field| object.get(*field).and_then(Value::as_u64))
        .max()
}

/// Reads standard reasoning configuration by protocol; when absent, do not infer the caller's need from the model catalog.
fn requested_reasoning(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> RequestedReasoning {
    // Responses accepts only the standard reasoning object and rejects the Chat top-level shorthand.
    if protocol == ApiProtocol::Responses && object.contains_key("reasoning_effort") {
        return RequestedReasoning::UnknownLevel;
    }

    // Chat accepts only standard reasoning_effort and rejects the Responses reasoning object.
    if protocol == ApiProtocol::ChatCompletions && object.contains_key("reasoning") {
        return if object.contains_key("reasoning_effort") {
            RequestedReasoning::Conflicting
        } else {
            RequestedReasoning::UnknownLevel
        };
    }

    // Read the standard fields for the current protocol and check ambiguity together afterward.
    let shorthand_value = object
        .get("reasoning_effort")
        .filter(|value| !value.is_null());
    let shorthand = shorthand_value
        .and_then(Value::as_str)
        .and_then(ReasoningLevel::from_wire);
    let reasoning_value = object.get("reasoning").filter(|value| !value.is_null());
    let object_effort = reasoning_value
        .and_then(Value::as_object)
        .and_then(|reasoning| reasoning.get("effort"));
    let object_level = object_effort
        .and_then(Value::as_str)
        .and_then(ReasoningLevel::from_wire);

    // Multiple configuration sources must agree; otherwise both Native and Bridge paths fail closed.
    if shorthand_value.is_some() && reasoning_value.is_some() {
        return match (shorthand, object_level) {
            (Some(left), Some(right)) if left == right => RequestedReasoning::Level(left),
            (Some(_), Some(_)) => RequestedReasoning::Conflicting,
            _ => RequestedReasoning::UnknownLevel,
        };
    }
    if shorthand_value.is_some() {
        return shorthand_value
            .and_then(Value::as_str)
            .and_then(ReasoningLevel::from_wire)
            .map(RequestedReasoning::Level)
            .unwrap_or(RequestedReasoning::UnknownLevel);
    }
    let Some(reasoning) = reasoning_value else {
        return RequestedReasoning::None;
    };
    // When the shorthand is absent, read effort from the Responses reasoning object.
    let Some(effort) = reasoning
        .as_object()
        .and_then(|reasoning| reasoning.get("effort"))
    else {
        return if reasoning.is_object() {
            RequestedReasoning::Unspecified
        } else {
            RequestedReasoning::UnknownLevel
        };
    };
    // Map known wire levels to the internal enum and fail closed on unknown values.
    effort
        .as_str()
        .and_then(ReasoningLevel::from_wire)
        .map(RequestedReasoning::Level)
        .unwrap_or(RequestedReasoning::UnknownLevel)
}

/// Extracts the exact function-tool choice mode, defaulting to `auto` when function tools exist.
fn requested_tool_choice(
    object: &serde_json::Map<String, Value>,
    has_function_tools: bool,
) -> (Option<ToolChoiceMode>, bool) {
    let Some(value) = object.get("tool_choice").filter(|value| !value.is_null()) else {
        return (has_function_tools.then_some(ToolChoiceMode::Auto), false);
    };
    match value {
        Value::String(value) => match value.as_str() {
            "none" => (Some(ToolChoiceMode::None), false),
            "auto" => (Some(ToolChoiceMode::Auto), false),
            "required" => (Some(ToolChoiceMode::Required), false),
            _ => (None, true),
        },
        Value::Object(choice) if choice.get("type").and_then(Value::as_str) == Some("function") => {
            let named = choice.get("name").and_then(Value::as_str).or_else(|| {
                choice
                    .get("function")
                    .and_then(Value::as_object)
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
            });
            (named.map(|_| ToolChoiceMode::Named), named.is_none())
        }
        _ => (None, true),
    }
}

/// Extracts one structured-output mode and strict-schema requirement from standard wire shapes.
fn requested_structured_output(
    object: &serde_json::Map<String, Value>,
) -> (Option<StructuredOutputMode>, bool, bool) {
    let mut mode = None;
    let mut unknown = false;
    // Parse every protocol-specific format location and reject conflicting or unknown modes.
    for format in [
        object.get("response_format"),
        object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format")),
    ]
    .into_iter()
    .flatten()
    {
        let (candidate, candidate_unknown) = structured_output_mode(format);
        unknown |= candidate_unknown;
        match candidate {
            Some(candidate) if mode.is_none() => mode = Some(candidate),
            Some(candidate) if mode == Some(candidate) => {}
            Some(_) => unknown = true,
            None => {}
        }
    }
    // Read strict JSON Schema markers without inferring a mode from an unrelated tool profile.
    let strict = [
        object.get("response_format"),
        object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format")),
    ]
    .into_iter()
    .flatten()
    .any(format_requests_strict);
    (mode, strict, unknown)
}

/// Maps one response-format object to the typed structured-output mode.
fn structured_output_mode(value: &Value) -> (Option<StructuredOutputMode>, bool) {
    let Some(format_type) = value
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
    else {
        return (None, true);
    };
    match format_type {
        "text" => (None, false),
        "json_object" => (Some(StructuredOutputMode::JsonObject), false),
        "json_schema" => (Some(StructuredOutputMode::JsonSchema), false),
        _ => (None, true),
    }
}

/// Returns whether one standard structured-output format requests strict JSON Schema.
fn format_requests_strict(value: &Value) -> bool {
    let is_json_schema = value.get("type").and_then(Value::as_str) == Some("json_schema");
    is_json_schema
        && (value.get("strict").and_then(Value::as_bool) == Some(true)
            || value
                .get("json_schema")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("strict"))
                .and_then(Value::as_bool)
                == Some(true))
}

/// Chat Completions places strict inside `function`, while Responses places it directly on the
/// function tool. Both wire shapes represent Structured Outputs and require `structured_outputs`.
fn tool_requests_strict_mode(tool: &Value) -> bool {
    tool.get("strict").and_then(Value::as_bool) == Some(true)
        || tool
            .get("function")
            .and_then(Value::as_object)
            .and_then(|function| function.get("strict"))
            .and_then(Value::as_bool)
            == Some(true)
}
