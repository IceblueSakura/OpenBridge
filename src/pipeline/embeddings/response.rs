//! Pure Embeddings success-response validation and downstream body projection.
//!
//! This module accepts already bounded bytes and immutable response metadata. It performs no body
//! reads, transport, observation, or downstream commit I/O.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use http::{HeaderMap, header::CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::EmbeddingEncoding;

/// Fail-closed marker for any bounded Embeddings response contract violation.
#[derive(Debug)]
pub(crate) struct EmbeddingResponseError;

/// Fully validated downstream body and low-cardinality usage extracted before commit.
pub(crate) struct ValidatedEmbeddingResponse {
    body: Vec<u8>,
    input_tokens: u64,
    total_tokens: u64,
}

impl ValidatedEmbeddingResponse {
    /// Returns usage from the fully validated success body.
    pub(crate) const fn usage(&self) -> (u64, u64) {
        (self.input_tokens, self.total_tokens)
    }

    /// Consumes the validation result and returns the projected downstream JSON body.
    pub(crate) fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingResponseBody {
    #[serde(default, rename = "id", skip_serializing)]
    _provider_id: Option<String>,
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

/// Validates one unambiguous JSON media type before ingress reads the success body.
pub(crate) fn validate_embedding_response_headers(
    headers: &HeaderMap,
) -> Result<(), EmbeddingResponseError> {
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return Err(EmbeddingResponseError);
    };
    if values.next().is_some() {
        return Err(EmbeddingResponseError);
    }
    let valid = value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"));
    valid.then_some(()).ok_or(EmbeddingResponseError)
}

/// Validates bounded Embeddings JSON and projects its trusted downstream model identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_embedding_response_body(
    body: &[u8],
    public_model: &str,
    upstream_model: &str,
    input_count: u32,
    encoding: EmbeddingEncoding,
    dimensions: u32,
    max_body_bytes: usize,
) -> Result<ValidatedEmbeddingResponse, EmbeddingResponseError> {
    let mut body: EmbeddingResponseBody =
        serde_json::from_slice(body).map_err(|_| EmbeddingResponseError)?;

    // Validate top-level identity, usage, and the one-to-one ordered data contract.
    if body.object != "list"
        || body.model != upstream_model
        || body.usage.total_tokens < body.usage.prompt_tokens
        || body.data.len() != usize::try_from(input_count).map_err(|_| EmbeddingResponseError)?
    {
        return Err(EmbeddingResponseError);
    }
    // Normalize the complete index set before validating one vector per logical input.
    body.data.sort_by_key(|item| item.index);
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
    let input_tokens = body.usage.prompt_tokens;
    let total_tokens = body.usage.total_tokens;
    body.model = public_model.to_owned();
    let body = serde_json::to_vec(&body).map_err(|_| EmbeddingResponseError)?;
    if body.len() > max_body_bytes {
        return Err(EmbeddingResponseError);
    }

    Ok(ValidatedEmbeddingResponse {
        body,
        input_tokens,
        total_tokens,
    })
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
