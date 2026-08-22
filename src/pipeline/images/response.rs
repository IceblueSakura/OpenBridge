//! Pure Images Generations success-response validation and downstream body projection.
//!
//! This module accepts already bounded bytes and immutable response metadata. It performs no body
//! reads, transport, observation, or downstream commit I/O.

use http::{HeaderMap, header::CONTENT_TYPE};
use serde::Serialize;
use serde_json::Value;

use crate::core::ImagesResponseFormat;

/// Fail-closed marker for any bounded Images response contract violation.
#[derive(Debug)]
pub(crate) struct ImagesResponseError;

/// Fully validated downstream body projected before commit.
pub(crate) struct ValidatedImagesResponse {
    body: Vec<u8>,
}

impl ValidatedImagesResponse {
    /// Consumes the validation result and returns the projected downstream JSON body.
    pub(crate) fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[derive(Serialize)]
struct ImagesResponseBody {
    created: u64,
    data: Vec<ImagesData>,
}

#[derive(Serialize)]
struct ImagesData {
    url: String,
}

/// Validates one unambiguous JSON media type before ingress reads the success body.
pub(crate) fn validate_images_response_headers(
    headers: &HeaderMap,
) -> Result<(), ImagesResponseError> {
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return Err(ImagesResponseError);
    };
    if values.next().is_some() {
        return Err(ImagesResponseError);
    }
    let valid = value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"));
    valid.then_some(()).ok_or(ImagesResponseError)
}

/// Validates bounded upstream JSON, extracts every generated image URL, and projects the OpenAI shape.
pub(crate) fn validate_images_response_body(
    body: &[u8],
    public_model: &str,
    expected_outputs: u32,
    response_format: ImagesResponseFormat,
    max_body_bytes: usize,
) -> Result<ValidatedImagesResponse, ImagesResponseError> {
    // Parse the upstream envelope and reject DashScope-native error payloads before interpreting output.
    let document: Value = serde_json::from_slice(body).map_err(|_| ImagesResponseError)?;
    if document.get("code").is_some() {
        return Err(ImagesResponseError);
    }
    let urls = document
        .get("output")
        .and_then(|output| output.get("choices"))
        .and_then(Value::as_array)
        .ok_or(ImagesResponseError)?
        .iter()
        .map(|choice| {
            choice
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(Value::as_array)
                .ok_or(ImagesResponseError)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .map(|part| {
            part.get("image")
                .and_then(Value::as_str)
                .filter(|url| !url.is_empty())
                .map(str::to_owned)
                .ok_or(ImagesResponseError)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Confirm the resolved output count and the only supported response format before projecting.
    let image_count = u32::try_from(urls.len()).map_err(|_| ImagesResponseError)?;
    if image_count != expected_outputs || response_format != ImagesResponseFormat::Url {
        return Err(ImagesResponseError);
    }
    let response = ImagesResponseBody {
        created: unix_timestamp(),
        data: urls.into_iter().map(|url| ImagesData { url }).collect(),
    };
    let projected = serde_json::to_vec(&response).map_err(|_| ImagesResponseError)?;
    if projected.len() > max_body_bytes {
        return Err(ImagesResponseError);
    }
    let _ = public_model;
    Ok(ValidatedImagesResponse { body: projected })
}

/// Returns the current unix timestamp in seconds without external clock dependency.
fn unix_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
