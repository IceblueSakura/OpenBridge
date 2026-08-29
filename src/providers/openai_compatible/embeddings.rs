//! Provider-scoped Embeddings request and response encoding translation.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Map, Value};

use crate::{
    core::{EmbeddingEncoding, EmbeddingEncodingPolicy},
    provider::AdapterError,
};

/// Narrows one downstream encoding request to the Provider's upstream wire.
pub(super) fn prepare_request_body(
    body: &mut Map<String, Value>,
    policy: EmbeddingEncodingPolicy,
) -> Result<(), AdapterError> {
    if policy == EmbeddingEncodingPolicy::Base64ViaFloat
        && body.get("encoding_format").and_then(Value::as_str) == Some("base64")
    {
        body.insert(
            "encoding_format".to_owned(),
            Value::String("float".to_owned()),
        );
    }
    Ok(())
}

/// Transcodes finite upstream float32-compatible vectors when Base64 was requested downstream.
pub(super) fn normalize_response_body(
    body: &[u8],
    requested_encoding: EmbeddingEncoding,
    policy: EmbeddingEncodingPolicy,
) -> Result<Vec<u8>, AdapterError> {
    if policy != EmbeddingEncodingPolicy::Base64ViaFloat
        || requested_encoding != EmbeddingEncoding::Base64
    {
        return Ok(body.to_vec());
    }

    let mut document: Value =
        serde_json::from_slice(body).map_err(|_| AdapterError::InvalidResponseBody)?;
    let data = document
        .get_mut("data")
        .and_then(Value::as_array_mut)
        .ok_or(AdapterError::InvalidResponseBody)?;
    for item in data {
        let embedding = item
            .get_mut("embedding")
            .ok_or(AdapterError::InvalidResponseBody)?;
        let values = embedding
            .as_array()
            .ok_or(AdapterError::InvalidResponseBody)?;
        let mut bytes = Vec::with_capacity(
            values
                .len()
                .checked_mul(std::mem::size_of::<f32>())
                .ok_or(AdapterError::InvalidResponseBody)?,
        );
        for value in values {
            let value = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or(AdapterError::InvalidResponseBody)? as f32;
            if !value.is_finite() {
                return Err(AdapterError::InvalidResponseBody);
            }
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        *embedding = Value::String(STANDARD.encode(bytes));
    }

    serde_json::to_vec(&document).map_err(|_| AdapterError::InvalidResponseBody)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn base64_via_float_rewrites_request_and_encodes_little_endian_f32() {
        let mut request = json!({"encoding_format":"base64"});
        prepare_request_body(
            request.as_object_mut().unwrap(),
            EmbeddingEncodingPolicy::Base64ViaFloat,
        )
        .unwrap();
        assert_eq!(request["encoding_format"], "float");

        let response = serde_json::to_vec(&json!({
            "data":[{"embedding":[0.5,-2.0]}]
        }))
        .unwrap();
        let response = normalize_response_body(
            &response,
            EmbeddingEncoding::Base64,
            EmbeddingEncodingPolicy::Base64ViaFloat,
        )
        .unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        let decoded = STANDARD
            .decode(response["data"][0]["embedding"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            decoded,
            [0.5_f32.to_le_bytes(), (-2.0_f32).to_le_bytes()].concat()
        );
    }

    #[test]
    fn preserve_policy_leaves_base64_wire_unchanged() {
        let mut request = json!({"encoding_format":"base64"});
        prepare_request_body(
            request.as_object_mut().unwrap(),
            EmbeddingEncodingPolicy::Preserve,
        )
        .unwrap();
        assert_eq!(request["encoding_format"], "base64");

        let response = serde_json::to_vec(&json!({
            "data":[{"embedding":"AQIDBA=="}]
        }))
        .unwrap();
        assert_eq!(
            normalize_response_body(
                &response,
                EmbeddingEncoding::Base64,
                EmbeddingEncodingPolicy::Preserve,
            )
            .unwrap(),
            response
        );
    }

    #[test]
    fn base64_via_float_rejects_values_outside_f32_range() {
        let response = serde_json::to_vec(&json!({
            "data":[{"embedding":[1.0e100]}]
        }))
        .unwrap();
        assert!(
            normalize_response_body(
                &response,
                EmbeddingEncoding::Base64,
                EmbeddingEncodingPolicy::Base64ViaFloat,
            )
            .is_err()
        );
    }
}
