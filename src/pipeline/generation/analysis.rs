//! Chat Completions and Responses request analysis.
//!
//! The analyzer extracts a fixed set of generation requirements and fails closed for protocol
//! positions that are modeled but not implemented. It never selects or reorders Routes.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    core::{
        ApiProtocol, ChatStreamUsage, GenerationRequestField, ResponseInclude, ToolChoiceMode,
        parse_chat_stream_usage,
    },
    registry::ReasoningLevel,
};

use super::super::{
    error::RequestPlanningError,
    types::{
        RequestRequirements, RequestedCapabilities, RequestedJsonSchemaStrictness,
        RequestedOutputTokens, RequestedReasoning, RequestedReasoningSummary,
        RequestedStructuredOutput,
    },
};
use super::instructions::{analyze_requested_instructions, validate_stateless_store};

mod audio;
mod file_input;
mod image_input;

use audio::analyze_audio;
use file_input::analyze_file_input;
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

    // Classify every top-level field before Native preservation or Bridge validation can observe it.
    let requested_parameters = classify_top_level_parameters(protocol, object)?;
    let requested_instructions = analyze_requested_instructions(protocol, object)?;
    validate_stateless_store(object)?;

    let is_streaming = object.get("stream").and_then(Value::as_bool) == Some(true);
    // Freeze the bounded Chat usage-tail request before preflight or candidate materialization.
    let chat_stream_usage = analyze_chat_stream_usage(protocol, object, is_streaming)?;

    // Block unimplemented protocol-specific fields before Route planning can enter Native or Bridge paths.
    reject_reserved_request_fields(protocol, object)?;

    // Derive the capabilities actually requested from protocol fields.
    let requested_output_tokens = requested_output_tokens(protocol, object);
    let response_includes = analyze_response_includes(protocol, object)?;
    let audio = analyze_audio(protocol, object)?;
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
    let structured_output = requested_structured_output(object);
    let function_tool_strict_schema = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(tool_requests_strict_mode));
    let requested_capabilities = RequestedCapabilities {
        streaming: is_streaming,
        function_tools: requests_function_calling,
        function_tool_choice,
        unknown_tool_choice,
        function_tool_strict_schema,
        parallel_tool_calls: requests_function_calling
            && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
        image_input: analyze_image_input(protocol, object)?,
        file_input: analyze_file_input(protocol, object)?,
        audio,
        structured_output,
        unmodeled_tools: requests_unmodeled_tools,
        reasoning: requested_reasoning(protocol, object),
        reasoning_summary: requested_reasoning_summary(protocol, object),
        previous_response_id: protocol == ApiProtocol::Responses
            && object
                .get("previous_response_id")
                .is_some_and(|value| !value.is_null()),
        background: protocol == ApiProtocol::Responses
            && object.get("background").and_then(Value::as_bool) == Some(true),
        response_includes,
    };
    // Freeze request facts so later Route planning does not reinterpret the body.
    Ok(RequestRequirements {
        public_model: public_model.to_owned(),
        protocol,
        is_streaming,
        chat_stream_usage,
        requested_output_tokens,
        requested_parameters,
        requested_instructions,
        requested_capabilities,
    })
}

/// Normalizes omitted and explicit no-op Chat stream options into one closed request fact.
fn analyze_chat_stream_usage(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
    is_streaming: bool,
) -> Result<ChatStreamUsage, RequestPlanningError> {
    if protocol != ApiProtocol::ChatCompletions {
        return Ok(ChatStreamUsage::NotRequested);
    }
    parse_chat_stream_usage(object, is_streaming).ok_or(RequestPlanningError::InvalidStreamOptions)
}

/// Classifies recognized fields and returns the active parameters owned by the fixed interface.
fn classify_top_level_parameters(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<std::collections::BTreeSet<GenerationRequestField>, RequestPlanningError> {
    // Reject the lexically first unknown field so error attribution is deterministic across maps.
    if let Some(unknown) = object
        .keys()
        .filter(|field| GenerationRequestField::from_wire(protocol, field).is_none())
        .min()
    {
        return Err(RequestPlanningError::UnknownParameter(unknown.clone()));
    }

    // Retain only fields whose typed value semantics require interface parameter ownership.
    Ok(object
        .iter()
        .filter_map(|(wire_name, value)| {
            let field = GenerationRequestField::from_wire(protocol, wire_name)
                .expect("unknown fields returned above");
            field.requires_interface_support(value).then_some(field)
        })
        .collect())
}

/// Returns a stable planning error when reserved Chat/Responses fields are used, preventing Native passthrough.
fn reject_reserved_request_fields(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<(), RequestPlanningError> {
    reserved_request_parameter(protocol, object)
        .map(|param| Err(RequestPlanningError::UnimplementedCapabilities { param }))
        .unwrap_or(Ok(()))
}

/// Returns the first standard top-level owner of one recognized but unimplemented feature.
fn reserved_request_parameter(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Option<&'static str> {
    let fields: &[&'static str] = match protocol {
        ApiProtocol::ChatCompletions => &[
            "prediction",
            "web_search_options",
            "prompt_cache_options",
            "prompt_cache_retention",
            "moderation",
        ],
        ApiProtocol::Responses => &[
            "modalities",
            "audio",
            "conversation",
            "prompt",
            "prompt_cache_options",
            "prompt_cache_retention",
            "context_management",
            "moderation",
        ],
    };
    if requests_reserved_tool(protocol, object) {
        return Some("tools");
    }
    if protocol == ApiProtocol::Responses
        && responses_input_contains_part_type(object, "input_audio")
    {
        return Some("input");
    }
    for field in fields {
        if (*field == "modalities" && array_field_contains(object, field, "audio"))
            || (*field != "modalities" && has_non_null_field(object, field))
        {
            return Some(field);
        }
    }
    if object.values().any(contains_prompt_cache_breakpoint) {
        return Some(match protocol {
            ApiProtocol::ChatCompletions => "messages",
            ApiProtocol::Responses => "input",
        });
    }
    None
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

/// Parses the Responses `include` field into a closed, registry-independent projection set.
fn analyze_response_includes(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<std::collections::BTreeSet<ResponseInclude>, RequestPlanningError> {
    // Treat omission and explicit null as an inactive value on either analyzed protocol.
    if protocol != ApiProtocol::Responses {
        return Ok(std::collections::BTreeSet::new());
    }
    let Some(value) = object.get("include").filter(|value| !value.is_null()) else {
        return Ok(std::collections::BTreeSet::new());
    };

    // Parse every exact wire value and fail closed for malformed or unknown projections.
    let values = value
        .as_array()
        .ok_or(RequestPlanningError::InvalidParameter("include"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .and_then(ResponseInclude::from_wire)
                .ok_or(RequestPlanningError::InvalidParameter("include"))
        })
        .collect()
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
fn requested_output_tokens(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Option<RequestedOutputTokens> {
    let fields: &[&'static str] = match protocol {
        ApiProtocol::ChatCompletions => &["max_completion_tokens", "max_tokens"],
        ApiProtocol::Responses => &["max_output_tokens"],
    };
    let mut selected = None;
    for param in fields {
        let Some(value) = object.get(*param).and_then(Value::as_u64) else {
            continue;
        };
        if selected
            .as_ref()
            .is_none_or(|current: &RequestedOutputTokens| value > current.value)
        {
            selected = Some(RequestedOutputTokens { value, param });
        }
    }
    selected
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

/// Classifies the only Responses summary request values currently preserved or consumed by OpenBridge.
fn requested_reasoning_summary(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> RequestedReasoningSummary {
    if protocol != ApiProtocol::Responses {
        return RequestedReasoningSummary::Absent;
    }
    let Some(summary) = object
        .get("reasoning")
        .and_then(Value::as_object)
        .and_then(|reasoning| reasoning.get("summary"))
    else {
        return RequestedReasoningSummary::Absent;
    };
    match summary {
        Value::Bool(false) => RequestedReasoningSummary::Disabled,
        Value::String(value) if value == "auto" => RequestedReasoningSummary::Auto,
        _ => RequestedReasoningSummary::Invalid,
    }
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

/// Extracts one closed structured-output requirement from the standard wire locations.
fn requested_structured_output(
    object: &serde_json::Map<String, Value>,
) -> RequestedStructuredOutput {
    let mut requested = RequestedStructuredOutput::Unconstrained;

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
        requested =
            merge_structured_output_requirements(requested, structured_output_requirement(format));
    }
    requested
}

/// Maps one response-format object to a complete structured-output requirement.
fn structured_output_requirement(value: &Value) -> RequestedStructuredOutput {
    let Some(format_type) = value
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
    else {
        return RequestedStructuredOutput::Unknown;
    };
    match format_type {
        "text" => RequestedStructuredOutput::Unconstrained,
        "json_object" => RequestedStructuredOutput::JsonObject,
        "json_schema" => RequestedStructuredOutput::JsonSchema(if format_requests_strict(value) {
            RequestedJsonSchemaStrictness::Strict
        } else {
            RequestedJsonSchemaStrictness::NonStrict
        }),
        _ => RequestedStructuredOutput::Unknown,
    }
}

/// Merges equivalent standard format locations and marks incompatible combinations unknown.
fn merge_structured_output_requirements(
    current: RequestedStructuredOutput,
    candidate: RequestedStructuredOutput,
) -> RequestedStructuredOutput {
    use RequestedJsonSchemaStrictness::{NonStrict, Strict};
    use RequestedStructuredOutput::{JsonObject, JsonSchema, Unconstrained, Unknown};

    match (current, candidate) {
        (Unknown, _) | (_, Unknown) => Unknown,
        (Unconstrained, value) | (value, Unconstrained) => value,
        (JsonObject, JsonObject) => JsonObject,
        (JsonSchema(Strict), JsonSchema(_)) | (JsonSchema(_), JsonSchema(Strict)) => {
            JsonSchema(Strict)
        }
        (JsonSchema(NonStrict), JsonSchema(NonStrict)) => JsonSchema(NonStrict),
        (JsonObject, JsonSchema(_)) | (JsonSchema(_), JsonObject) => Unknown,
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

/// Detects strict function-schema requests without conflating them with response Structured Output.
fn tool_requests_strict_mode(tool: &Value) -> bool {
    tool.get("strict").and_then(Value::as_bool) == Some(true)
        || tool
            .get("function")
            .and_then(Value::as_object)
            .and_then(|function| function.get("strict"))
            .and_then(Value::as_bool)
            == Some(true)
}
