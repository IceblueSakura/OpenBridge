//! Strict Images Generations request analysis.
//!
//! The analyzer accepts only the closed OpenAI Images wire contract and retains structural facts
//! needed by fixed-interface preflight and bounded response validation. Prompt text, sizes, and
//! user identifiers are never retained outside the preserved request body.

use bytes::Bytes;
use serde_json::Value;

use crate::core::ImagesResponseFormat;

use super::super::{
    error::ImagesRequestError, types::ImagesRequestRequirements, types::ImagesRequestedSize,
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
    const ALLOWED_FIELDS: &[&str] = &["model", "prompt", "n", "size", "response_format", "user"];
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
        None => None,
        Some(Value::Number(value)) if value.is_u64() => Some(
            u32::try_from(value.as_u64().expect("checked positive integer"))
                .map_err(|_| ImagesRequestError::invalid(Some("n")))?,
        ),
        Some(_) => return Err(ImagesRequestError::invalid(Some("n"))),
    };
    let requested_size = match object.get("size") {
        None => None,
        Some(Value::String(value)) => Some(parse_images_size(value)?),
        Some(_) => return Err(ImagesRequestError::invalid(Some("size"))),
    };
    let requested_response_format = match object.get("response_format") {
        None => None,
        Some(Value::String(value)) if value == "url" => Some(ImagesResponseFormat::Url),
        Some(_) => return Err(ImagesRequestError::invalid(Some("response_format"))),
    };
    let user_present = match object.get("user") {
        None => false,
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
        user_present,
    })
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
    Ok(ImagesRequestedSize { width, height })
}
