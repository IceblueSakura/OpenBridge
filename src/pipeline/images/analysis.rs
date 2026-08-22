//! Strict Images Generations request analysis.
//!
//! The analyzer accepts only the closed OpenAI Images wire contract and retains structural facts
//! needed by fixed-interface preflight and bounded response validation. Prompt text, sizes, and
//! user identifiers are never retained outside the preserved request body.

use bytes::Bytes;
use serde_json::Value;

use crate::core::{DashScopePromptExtendMode, ImagesOutputFormat, ImagesResponseFormat};

use super::super::{
    error::ImagesRequestError,
    types::{
        DashScopeImagesRequestRequirements, ImagesRequestRequirements, ImagesRequestedSize,
        ImagesUnsupportedStandardField,
    },
};

/// Parses one strict Images Generations request into registry-independent facts.
pub fn analyze_images_request(
    body: &Bytes,
) -> Result<ImagesRequestRequirements, ImagesRequestError> {
    // Parse exactly one JSON object and reject fields outside the initial Images contract.
    let document: Value =
        serde_json::from_slice(body).map_err(|_| ImagesRequestError::invalid(None))?;
    let object = document
        .as_object()
        .ok_or_else(|| ImagesRequestError::invalid(None))?;
    const ALLOWED_FIELDS: &[&str] = &[
        "background",
        "enable_thinking",
        "model",
        "moderation",
        "negative_prompt",
        "prompt",
        "n",
        "output_compression",
        "output_format",
        "partial_images",
        "prompt_extend",
        "prompt_extend_mode",
        "quality",
        "response_format",
        "seed",
        "size",
        "stream",
        "style",
        "user",
        "watermark",
    ];
    if object
        .keys()
        .any(|field| !ALLOWED_FIELDS.contains(&field.as_str()))
    {
        return Err(ImagesRequestError::invalid(None));
    }

    // Extract the stable Public Model and the single non-blank prompt without coercion.
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.trim().is_empty())
        .ok_or_else(|| ImagesRequestError::invalid(Some("model")))?;
    let prompt = object
        .get("prompt")
        .and_then(Value::as_str)
        .filter(|prompt| !prompt.trim().is_empty())
        .ok_or_else(|| ImagesRequestError::invalid(Some("prompt")))?;
    let prompt_length =
        u32::try_from(prompt.len()).map_err(|_| ImagesRequestError::invalid(Some("prompt")))?;

    // Parse optional output count, size, response-format, and user fields using standard wire types.
    let requested_outputs = match object.get("n") {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) if value.is_u64() => Some(
            u32::try_from(value.as_u64().expect("checked positive integer"))
                .map_err(|_| ImagesRequestError::invalid(Some("n")))?,
        ),
        Some(_) => return Err(ImagesRequestError::invalid(Some("n"))),
    };
    let requested_size = match object.get("size") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "auto" => Some(ImagesRequestedSize::Auto),
        Some(Value::String(value)) => Some(parse_images_size(value)?),
        Some(_) => return Err(ImagesRequestError::invalid(Some("size"))),
    };
    let requested_response_format = match object.get("response_format") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "url" => Some(ImagesResponseFormat::Url),
        Some(Value::String(value)) if value == "b64_json" => Some(ImagesResponseFormat::B64Json),
        Some(_) => return Err(ImagesRequestError::invalid(Some("response_format"))),
    };
    let requested_output_format = match object.get("output_format") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "png" => Some(ImagesOutputFormat::Png),
        Some(Value::String(value)) if value == "jpeg" => Some(ImagesOutputFormat::Jpeg),
        Some(Value::String(value)) if value == "webp" => Some(ImagesOutputFormat::Webp),
        Some(_) => return Err(ImagesRequestError::invalid(Some("output_format"))),
    };
    let requested_stream = match object.get("stream") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => return Err(ImagesRequestError::invalid(Some("stream"))),
    };
    let mut unsupported_standard_fields = Vec::new();
    parse_string_enum_field(
        object,
        "background",
        &["transparent", "opaque", "auto"],
        ImagesUnsupportedStandardField::Background,
        &mut unsupported_standard_fields,
    )?;
    parse_string_enum_field(
        object,
        "moderation",
        &["low", "auto"],
        ImagesUnsupportedStandardField::Moderation,
        &mut unsupported_standard_fields,
    )?;
    parse_bounded_integer_field(
        object,
        "output_compression",
        100,
        ImagesUnsupportedStandardField::OutputCompression,
        &mut unsupported_standard_fields,
    )?;
    parse_bounded_integer_field(
        object,
        "partial_images",
        3,
        ImagesUnsupportedStandardField::PartialImages,
        &mut unsupported_standard_fields,
    )?;
    parse_string_enum_field(
        object,
        "quality",
        &["standard", "hd", "low", "medium", "high", "auto"],
        ImagesUnsupportedStandardField::Quality,
        &mut unsupported_standard_fields,
    )?;
    parse_string_enum_field(
        object,
        "style",
        &["vivid", "natural"],
        ImagesUnsupportedStandardField::Style,
        &mut unsupported_standard_fields,
    )?;
    let dashscope = parse_dashscope_extensions(object)?;
    let user_present = match object.get("user") {
        None | Some(Value::Null) => false,
        Some(Value::String(_)) => true,
        Some(_) => return Err(ImagesRequestError::invalid(Some("user"))),
    };

    // Freeze only structural facts required by fixed-interface preflight and response validation.
    Ok(ImagesRequestRequirements {
        public_model: public_model.to_owned(),
        prompt_length,
        requested_outputs,
        requested_size,
        requested_response_format,
        requested_output_format,
        requested_stream,
        unsupported_standard_fields,
        dashscope,
        user_present,
    })
}

/// Parses DashScope-compatible top-level fields used through OpenAI SDK `extra_body`.
fn parse_dashscope_extensions(
    object: &serde_json::Map<String, Value>,
) -> Result<DashScopeImagesRequestRequirements, ImagesRequestError> {
    let prompt_extend = parse_nullable_boolean(object, "prompt_extend")?;
    let prompt_extend_mode = match object.get("prompt_extend_mode") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "direct" => Some(DashScopePromptExtendMode::Direct),
        Some(Value::String(value)) if value == "agent" => Some(DashScopePromptExtendMode::Agent),
        Some(_) => return Err(ImagesRequestError::invalid(Some("prompt_extend_mode"))),
    };
    let enable_thinking = parse_nullable_boolean(object, "enable_thinking")?;
    if prompt_extend == Some(false) {
        if prompt_extend_mode.is_some() {
            return Err(ImagesRequestError::invalid(Some("prompt_extend_mode")));
        }
        if enable_thinking.is_some() {
            return Err(ImagesRequestError::invalid(Some("enable_thinking")));
        }
    }
    let negative_prompt_present = match object.get("negative_prompt") {
        None | Some(Value::Null) => false,
        Some(Value::String(value)) if !value.trim().is_empty() => true,
        Some(_) => return Err(ImagesRequestError::invalid(Some("negative_prompt"))),
    };
    let seed = match object.get("seed") {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) if value.is_u64() => Some(
            u32::try_from(value.as_u64().expect("checked unsigned integer"))
                .map_err(|_| ImagesRequestError::invalid(Some("seed")))?,
        ),
        Some(_) => return Err(ImagesRequestError::invalid(Some("seed"))),
    };
    Ok(DashScopeImagesRequestRequirements {
        prompt_extend,
        prompt_extend_mode,
        enable_thinking,
        negative_prompt_present,
        seed,
        watermark: parse_nullable_boolean(object, "watermark")?,
    })
}

/// Parses one nullable boolean extension field without coercion.
fn parse_nullable_boolean(
    object: &serde_json::Map<String, Value>,
    parameter: &'static str,
) -> Result<Option<bool>, ImagesRequestError> {
    match object.get(parameter) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(ImagesRequestError::invalid(Some(parameter))),
    }
}

/// Parses one nullable standard enum field and records valid-but-unsupported presence.
fn parse_string_enum_field(
    object: &serde_json::Map<String, Value>,
    parameter: &'static str,
    allowed: &[&str],
    field: ImagesUnsupportedStandardField,
    requested: &mut Vec<ImagesUnsupportedStandardField>,
) -> Result<(), ImagesRequestError> {
    match object.get(parameter) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(value)) if allowed.contains(&value.as_str()) => {
            requested.push(field);
            Ok(())
        }
        Some(_) => Err(ImagesRequestError::invalid(Some(parameter))),
    }
}

/// Parses one nullable standard non-negative integer bounded by its OpenAI wire maximum.
fn parse_bounded_integer_field(
    object: &serde_json::Map<String, Value>,
    parameter: &'static str,
    maximum: u64,
    field: ImagesUnsupportedStandardField,
    requested: &mut Vec<ImagesUnsupportedStandardField>,
) -> Result<(), ImagesRequestError> {
    match object.get(parameter) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Number(value)) if value.as_u64().is_some_and(|value| value <= maximum) => {
            requested.push(field);
            Ok(())
        }
        Some(_) => Err(ImagesRequestError::invalid(Some(parameter))),
    }
}

/// Parses one OpenAI `WxH` size string into positive pixel dimensions.
fn parse_images_size(value: &str) -> Result<ImagesRequestedSize, ImagesRequestError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(ImagesRequestError::invalid(Some("size")));
    };
    if width.is_empty()
        || height.is_empty()
        || width.starts_with('0')
        || height.starts_with('0')
        || !width.bytes().all(|byte| byte.is_ascii_digit())
        || !height.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ImagesRequestError::invalid(Some("size")));
    }
    let width = width
        .parse::<u32>()
        .map_err(|_| ImagesRequestError::invalid(Some("size")))?;
    let height = height
        .parse::<u32>()
        .map_err(|_| ImagesRequestError::invalid(Some("size")))?;
    if width == 0 || height == 0 {
        return Err(ImagesRequestError::invalid(Some("size")));
    }
    Ok(ImagesRequestedSize::Exact { width, height })
}
