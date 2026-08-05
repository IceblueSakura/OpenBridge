//! Extracts registry-independent request facts from OpenAI-compatible JSON.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    core::{
        ApiProtocol, EmbeddingEncoding, EmbeddingInputForm, GenerationCapabilities, ReasoningOutput,
    },
    registry::ReasoningLevel,
};

use super::{
    error::{EmbeddingRequestError, RequestPlanningError},
    types::{
        EmbeddingRequestRequirements, RequestRequirements, RequestedCapabilities,
        RequestedReasoning,
    },
};

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
    let requests_function_calling = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(is_function_tool));
    let requests_unmodeled_tools =
        object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| {
                tools
                    .iter()
                    .any(|tool| !is_function_tool(tool) && !is_reserved_tool(protocol, tool))
            });
    let requested_capabilities = RequestedCapabilities {
        generation: GenerationCapabilities {
            enabled: false,
            streaming: is_streaming,
            function_calling: requests_function_calling,
            parallel_tool_calls: requests_function_calling
                && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
            image_input: requests_image_input(protocol, object),
            structured_outputs: requests_structured_outputs(object),
            store: object.get("store").and_then(Value::as_bool) == Some(true),
            reasoning_output: ReasoningOutput::Unknown,
        },
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

/// Parses one strict Embeddings Create request into registry-independent facts.
///
/// The parser accepts only the current endpoint fields and four closed input shapes. It records
/// counts rather than retaining business text or token values outside the original request body.
pub fn analyze_embedding_request(
    body: &Bytes,
) -> Result<EmbeddingRequestRequirements, EmbeddingRequestError> {
    // Parse exactly one JSON object and reject fields outside the initial Embeddings contract.
    let document: Value =
        serde_json::from_slice(body).map_err(|_| EmbeddingRequestError::invalid(None))?;
    let object = document
        .as_object()
        .ok_or_else(|| EmbeddingRequestError::invalid(None))?;
    const ALLOWED_FIELDS: &[&str] = &["model", "input", "encoding_format", "dimensions", "user"];
    if object
        .keys()
        .any(|field| !ALLOWED_FIELDS.contains(&field.as_str()))
    {
        return Err(EmbeddingRequestError::invalid(None));
    }

    // Extract the stable Public Model and analyze the non-empty input union without coercion.
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.trim().is_empty())
        .ok_or_else(|| EmbeddingRequestError::invalid(Some("model")))?;
    let input = analyze_embedding_input(
        object
            .get("input")
            .ok_or_else(|| EmbeddingRequestError::invalid(Some("input")))?,
    )?;

    // Parse optional enum, dimension, and user fields using only their standard wire types.
    let requested_encoding = match object.get("encoding_format") {
        None => None,
        Some(Value::String(value)) if value == "float" => Some(EmbeddingEncoding::Float),
        Some(Value::String(value)) if value == "base64" => Some(EmbeddingEncoding::Base64),
        Some(_) => return Err(EmbeddingRequestError::invalid(Some("encoding_format"))),
    };
    let requested_dimensions = match object.get("dimensions") {
        None => None,
        Some(value) => value
            .as_u64()
            .filter(|value| *value > 0)
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| EmbeddingRequestError::invalid(Some("dimensions")))?,
    };
    let user_present = match object.get("user") {
        None => false,
        Some(Value::String(_)) => true,
        Some(_) => return Err(EmbeddingRequestError::invalid(Some("user"))),
    };

    // Freeze only structural facts required by fixed-interface preflight and response validation.
    Ok(EmbeddingRequestRequirements {
        public_model: public_model.to_owned(),
        input_form: input.form,
        input_count: input.count,
        token_counts: input.token_counts,
        requested_encoding,
        requested_dimensions,
        user_present,
    })
}

struct EmbeddingInputAnalysis {
    form: EmbeddingInputForm,
    count: u32,
    token_counts: Option<Vec<u32>>,
}

/// Classifies one non-empty input union and computes exact token-array counts.
fn analyze_embedding_input(value: &Value) -> Result<EmbeddingInputAnalysis, EmbeddingRequestError> {
    // Accept one non-empty string as one logical input without estimating tokens.
    if let Some(value) = value.as_str() {
        if value.is_empty() {
            return Err(EmbeddingRequestError::invalid(Some("input")));
        }
        return Ok(EmbeddingInputAnalysis {
            form: EmbeddingInputForm::String,
            count: 1,
            token_counts: None,
        });
    }

    // Require a non-empty array before discriminating string, token, and nested-token shapes.
    let values = value
        .as_array()
        .ok_or_else(|| EmbeddingRequestError::invalid(Some("input")))?;
    if values.is_empty() {
        return Err(EmbeddingRequestError::invalid(Some("input")));
    }
    let count =
        u32::try_from(values.len()).map_err(|_| EmbeddingRequestError::invalid(Some("input")))?;

    // Accept a homogeneous array of non-empty strings as a batch of string inputs.
    if values.iter().all(Value::is_string) {
        if values
            .iter()
            .any(|value| value.as_str().is_none_or(str::is_empty))
        {
            return Err(EmbeddingRequestError::invalid(Some("input")));
        }
        return Ok(EmbeddingInputAnalysis {
            form: EmbeddingInputForm::StringArray,
            count,
            token_counts: None,
        });
    }

    // Accept a homogeneous token-ID array as one input and record its exact length.
    if values
        .iter()
        .all(|value| parse_embedding_token(value).is_some())
    {
        return Ok(EmbeddingInputAnalysis {
            form: EmbeddingInputForm::TokenArray,
            count: 1,
            token_counts: Some(vec![count]),
        });
    }

    // Accept a homogeneous non-empty array of valid token-ID arrays and record each length.
    if values.iter().all(Value::is_array) {
        let token_counts = values
            .iter()
            .map(|value| {
                let tokens = value
                    .as_array()
                    .ok_or_else(|| EmbeddingRequestError::invalid(Some("input")))?;
                if tokens.is_empty()
                    || tokens
                        .iter()
                        .any(|token| parse_embedding_token(token).is_none())
                {
                    return Err(EmbeddingRequestError::invalid(Some("input")));
                }
                u32::try_from(tokens.len())
                    .map_err(|_| EmbeddingRequestError::invalid(Some("input")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(EmbeddingInputAnalysis {
            form: EmbeddingInputForm::TokenArrayArray,
            count,
            token_counts: Some(token_counts),
        });
    }

    // Reject mixed, ambiguous, scalar, object, Boolean, null, negative, or non-integer shapes.
    Err(EmbeddingRequestError::invalid(Some("input")))
}

/// Accepts one non-negative token ID representable by the initial wire contract.
fn parse_embedding_token(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|value| u32::try_from(value).ok())
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
        || chat_messages_contain_part_type(object, "input_audio")
        || chat_messages_contain_part_type(object, "file")
        || array_field_contains(object, "modalities", "audio")
        || has_non_null_field(object, "audio")
        || has_non_null_field(object, "prediction")
        || has_non_null_field(object, "web_search_options")
        || requests_prompt_caching(object)
        || has_non_null_field(object, "moderation")
        || object.get("logprobs").and_then(Value::as_bool) == Some(true)
        || integer_field_exceeds(object, "top_logprobs", 0)
        || integer_field_exceeds(object, "n", 1)
}

/// Returns whether a Responses request uses a capability reserved only in the definition.
fn requests_reserved_responses_capability(object: &serde_json::Map<String, Value>) -> bool {
    requests_reserved_tool(ApiProtocol::Responses, object)
        || responses_input_contains_part_type(object, "input_file")
        || has_non_null_field(object, "conversation")
        || has_non_null_field(object, "prompt")
        || requests_prompt_caching(object)
        || has_non_null_field(object, "context_management")
        || has_non_null_field(object, "include")
        || has_non_null_field(object, "moderation")
        || integer_field_exceeds(object, "top_logprobs", 0)
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

/// Returns whether a non-negative integer field exceeds the given capability ceiling.
fn integer_field_exceeds(
    object: &serde_json::Map<String, Value>,
    field: &str,
    baseline: u64,
) -> bool {
    object
        .get(field)
        .and_then(Value::as_u64)
        .is_some_and(|value| value > baseline)
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

/// Identifies image content parts only within OpenAI-compatible message/input items; it does not calculate tokens on the hot path.
///
/// `image_url` (Chat) and `input_image` (Responses) are protocol fields. Unknown parts are passed
/// through natively, so visual capability cannot be inferred from a same-named `type` elsewhere in JSON.
fn requests_image_input(protocol: ApiProtocol, object: &serde_json::Map<String, Value>) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => object
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| content_contains_part_type(message.get("content"), "image_url"))
            }),
        ApiProtocol::Responses => {
            object
                .get("input")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("input_image")
                            || content_contains_part_type(item.get("content"), "input_image")
                    })
                })
        }
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

/// `function_calling` covers only OpenAI JSON Schema function tools. Built-in and custom tools need
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

/// Identifies a structured-output request through response format, text format, or a strict function tool.
fn requests_structured_outputs(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("response_format")
        .is_some_and(is_non_text_format)
        || object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format"))
            .is_some_and(is_non_text_format)
        || object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| tools.iter().any(tool_requests_strict_mode))
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

/// Returns whether a format object explicitly requires non-plain-text output.
fn is_non_text_format(format: &Value) -> bool {
    format
        .as_object()
        .and_then(|format| format.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|format_type| format_type != "text")
}
