//! Bounded validation and Public Model projection for Native Embeddings success responses.
//!
//! The complete upstream body is validated before downstream commit. Business vectors are held
//! only in the bounded response value, are never logged, and are serialized without conversion.

use axum::{body::to_bytes, response::Response};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use http::{HeaderMap, header::CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    core::EmbeddingEncoding, ingress::response::filtered_upstream_headers,
    transport::upstream::UpstreamResponse,
};

/// Fail-closed marker for any bounded Embeddings response contract violation.
#[derive(Debug)]
pub(super) struct EmbeddingResponseError;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingResponseBody {
    object: String,
    data: Vec<EmbeddingData>,
    model: String,
    usage: EmbeddingUsage,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingData {
    object: String,
    embedding: Value,
    index: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingUsage {
    prompt_tokens: u64,
    total_tokens: u64,
}

/// Validates one successful Embeddings response and projects its model for downstream delivery.
#[allow(clippy::too_many_arguments)]
pub(super) async fn validated_embedding_response(
    upstream: UpstreamResponse,
    public_model: &str,
    upstream_model: &str,
    input_count: u32,
    encoding: EmbeddingEncoding,
    dimensions: u32,
    max_body_bytes: usize,
) -> Result<Response, EmbeddingResponseError> {
    // Require one JSON media type before reading or interpreting the successful body.
    if !has_json_content_type(upstream.headers()) {
        return Err(EmbeddingResponseError);
    }
    let status = upstream.status();
    let headers = filtered_upstream_headers(upstream.headers());

    // Read the entire response under its independent pre-commit memory boundary.
    let body = to_bytes(upstream.into_body(), max_body_bytes)
        .await
        .map_err(|_| EmbeddingResponseError)?;
    let mut body: EmbeddingResponseBody =
        serde_json::from_slice(&body).map_err(|_| EmbeddingResponseError)?;

    // Validate top-level identity, usage, and the one-to-one ordered data contract.
    if body.object != "list"
        || body.model != upstream_model
        || body.usage.total_tokens < body.usage.prompt_tokens
        || body.data.len() != usize::try_from(input_count).map_err(|_| EmbeddingResponseError)?
    {
        return Err(EmbeddingResponseError);
    }
    for (position, item) in body.data.iter().enumerate() {
        let expected_index = u32::try_from(position).map_err(|_| EmbeddingResponseError)?;
        if item.object != "embedding"
            || item.index != expected_index
            || !valid_embedding_value(&item.embedding, encoding, dimensions)
        {
            return Err(EmbeddingResponseError);
        }
    }

    // Replace only the trusted upstream model identity and preserve data/usage value semantics.
    body.model = public_model.to_owned();
    let body = serde_json::to_vec(&body).map_err(|_| EmbeddingResponseError)?;
    if body.len() > max_body_bytes {
        return Err(EmbeddingResponseError);
    }

    // Commit the fully validated, bounded JSON response with only allowlisted upstream headers.
    let mut response = Response::builder()
        .status(status)
        .body(axum::body::Body::from(body))
        .expect("validated upstream status builds a response");
    response.headers_mut().extend(headers);
    Ok(response)
}

/// Returns whether the response carries one compatible JSON media type.
fn has_json_content_type(headers: &HeaderMap) -> bool {
    // Reject missing or duplicate values so the validator has one unambiguous representation.
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }

    // Accept optional media-type parameters while requiring application/json itself.
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

/// Validates one float array or standard base64 float32 payload against the effective dimension.
fn valid_embedding_value(value: &Value, encoding: EmbeddingEncoding, dimensions: u32) -> bool {
    let Ok(dimensions) = usize::try_from(dimensions) else {
        return false;
    };
    match encoding {
        EmbeddingEncoding::Float => value.as_array().is_some_and(|values| {
            values.len() == dimensions
                && values
                .iter()
                .all(|value| value.as_f64().is_some_and(f64::is_finite))
        }),
        EmbeddingEncoding::Base64 => {
            let Some(value) = value.as_str() else {
                return false;
            };
            let Some(expected_bytes) = dimensions.checked_mul(std::mem::size_of::<f32>()) else {
                return false;
            };
            STANDARD
                .decode(value)
                .is_ok_and(|decoded| decoded.len() == expected_bytes)
        }
    }
}
