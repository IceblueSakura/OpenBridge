//! Chat Completions and Responses request analysis.
//!
//! The analyzer extracts a fixed set of generation requirements and fails closed for protocol
//! positions that are modeled but not implemented. It never selects or reorders Routes.

use std::net::{Ipv4Addr, Ipv6Addr};

use bytes::Bytes;
use serde_json::Value;
use url::{Host, Url};

use crate::{
    core::{ApiProtocol, ImageDetail, ImageInputSource, ImageMediaType},
    registry::ReasoningLevel,
};

use super::super::{
    error::RequestPlanningError,
    types::{
        ImageInputRequirements, RequestRequirements, RequestedCapabilities, RequestedReasoning,
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
        streaming: is_streaming,
        function_calling: requests_function_calling,
        parallel_tool_calls: requests_function_calling
            && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
        image_input: analyze_image_input(protocol, object)?,
        structured_outputs: requests_structured_outputs(object),
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

/// Parses image content parts only from their protocol-defined user-message positions.
fn analyze_image_input(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<ImageInputRequirements>, RequestPlanningError> {
    match protocol {
        ApiProtocol::ChatCompletions => analyze_chat_images(object),
        ApiProtocol::Responses => analyze_responses_images(object),
    }
}

/// Parses Chat `image_url` parts from user messages and freezes their source facts.
fn analyze_chat_images(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<ImageInputRequirements>, RequestPlanningError> {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut requirements = ImageInputRequirements::default();
    for message in messages {
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("image_url") {
                continue;
            }

            // Image content is valid only in a standard user message.
            if message.get("role").and_then(Value::as_str) != Some("user") {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            let image = part
                .get("image_url")
                .and_then(Value::as_object)
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
            let image_url = image
                .get("url")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;

            // Freeze the optional nested Chat detail and the URL/data-URL source.
            ingest_detail(image.get("detail"), &mut requirements)?;
            ingest_image_reference(image_url, &mut requirements)?;
        }
    }
    if requirements.part_count == 0 {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

/// Parses Responses `input_image` parts from user input messages and freezes their source facts.
fn analyze_responses_images(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<ImageInputRequirements>, RequestPlanningError> {
    let Some(items) = object.get("input").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut requirements = ImageInputRequirements::default();
    for item in items {
        // A content part cannot appear as a standalone Responses input item.
        if item.get("type").and_then(Value::as_str) == Some("input_image") {
            return Err(RequestPlanningError::InvalidMultimodalInput);
        }
        let Some(parts) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("input_image") {
                continue;
            }

            // Current Native image input accepts only a standard user message content part.
            if item.get("role").and_then(Value::as_str) != Some("user") {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            let image_url = part.get("image_url").filter(|value| !value.is_null());
            let file_id = part.get("file_id").filter(|value| !value.is_null());
            if usize::from(image_url.is_some()) + usize::from(file_id.is_some()) != 1 {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }

            // Freeze detail and exactly one standard Responses image source.
            ingest_detail(part.get("detail"), &mut requirements)?;
            if let Some(image_url) = image_url {
                let image_url = image_url
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
                ingest_image_reference(image_url, &mut requirements)?;
            } else {
                let _file_id = file_id
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
                increment_part_count(&mut requirements)?;
                requirements.sources.insert(ImageInputSource::FileId);
            }
        }
    }
    if requirements.part_count == 0 {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

/// Parses an optional explicit image-detail value.
fn ingest_detail(
    value: Option<&Value>,
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    let Some(value) = value else {
        return Ok(());
    };
    let detail = value
        .as_str()
        .and_then(ImageDetail::from_wire)
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    requirements.details.insert(detail);
    Ok(())
}

/// Classifies and validates one remote URL or inline image data URL.
fn ingest_image_reference(
    value: &str,
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    increment_part_count(requirements)?;
    if value.starts_with("data:") {
        return ingest_data_url(value, requirements);
    }

    // Validate only the inbound URL syntax; the Provider still owns DNS, redirects, and download limits.
    validate_remote_image_url(value)?;
    let length = u32::try_from(value.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;
    requirements.sources.insert(ImageInputSource::RemoteUrl);
    requirements.max_url_length = requirements.max_url_length.max(length);
    Ok(())
}

/// Validates one canonical Base64 data URL and records encoded and decoded byte counts.
fn ingest_data_url(
    value: &str,
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    // Split the exact data-URL media declaration from its non-empty Base64 payload.
    let (metadata, payload) = value
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let media_type = metadata
        .strip_suffix(";base64")
        .filter(|value| !value.is_empty())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if payload.is_empty() {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let encoded_bytes = u32::try_from(payload.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;

    // Validate canonical standard Base64 and derive its exact decoded size without allocating media bytes.
    let decoded_bytes = canonical_base64_decoded_bytes(payload)?;

    // Accumulate bounded request facts without retaining media bytes or the original URL.
    requirements.sources.insert(ImageInputSource::DataUrl);
    match ImageMediaType::from_wire(media_type) {
        Some(media_type) => {
            requirements.media_types.insert(media_type);
        }
        None => requirements.unsupported_media_type = true,
    }
    requirements.max_inline_encoded_bytes =
        requirements.max_inline_encoded_bytes.max(encoded_bytes);
    requirements.max_inline_decoded_bytes =
        requirements.max_inline_decoded_bytes.max(decoded_bytes);
    requirements.total_inline_encoded_bytes = requirements
        .total_inline_encoded_bytes
        .checked_add(encoded_bytes)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    requirements.total_inline_decoded_bytes = requirements
        .total_inline_decoded_bytes
        .checked_add(decoded_bytes)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    Ok(())
}

/// Validates canonical padded or unpadded standard Base64 and returns its exact decoded byte count.
fn canonical_base64_decoded_bytes(payload: &str) -> Result<u32, RequestPlanningError> {
    let bytes = payload.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }

    // Locate at most two terminal padding bytes and reject padding in the encoded body.
    let padding = if bytes.ends_with(b"==") {
        2
    } else if bytes.ends_with(b"=") {
        1
    } else {
        0
    };
    let content_length = bytes.len() - padding;
    if content_length == 0
        || bytes[..content_length]
            .iter()
            .any(|value| base64_sextet(*value).is_none())
        || bytes[content_length..].iter().any(|value| *value != b'=')
    {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }

    // Require zero unused bits so alternate encodings of the same bytes cannot pass as canonical.
    let final_sextet = base64_sextet(bytes[content_length - 1])
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if (padding == 2 && final_sextet & 0b1111 != 0) || (padding == 1 && final_sextet & 0b11 != 0) {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }

    // Compute the decoded size only after the alphabet, padding, and unused-bit checks succeed.
    let decoded = bytes
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    u32::try_from(decoded).map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)
}

/// Returns the six-bit value for one standard Base64 alphabet byte.
const fn base64_sextet(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Increments the total image-part count with checked arithmetic.
fn increment_part_count(
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    requirements.part_count = requirements
        .part_count
        .checked_add(1)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    Ok(())
}

/// Applies the inbound absolute-HTTPS and local-address policy without fetching the URL.
fn validate_remote_image_url(value: &str) -> Result<(), RequestPlanningError> {
    let url = Url::parse(value).map_err(|_| RequestPlanningError::InvalidMultimodalInput)?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let host = url
        .host()
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let blocked = match host {
        Host::Domain(domain) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost")
        }
        Host::Ipv4(address) => !is_public_ipv4(address),
        Host::Ipv6(address) => !is_public_ipv6(address),
    };
    if blocked {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    Ok(())
}

/// Returns whether an IPv4 literal is outside local, reserved, documentation, and multicast ranges.
fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_multicast()
        || first == 0
        || (first == 100 && (64..=127).contains(&second))
        || (first == 198 && (18..=19).contains(&second))
        || first >= 240)
}

/// Returns whether an IPv6 literal is outside local, reserved, documentation, and multicast ranges.
fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
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
