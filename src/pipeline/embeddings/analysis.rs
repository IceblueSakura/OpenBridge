//! Strict Embeddings Create request analysis.
//!
//! The analyzer accepts the closed initial wire contract and retains only structural facts needed
//! by fixed-interface preflight and bounded response validation.

use bytes::Bytes;
use serde_json::Value;

use crate::core::{EmbeddingEncoding, EmbeddingInputForm};

use super::super::{error::EmbeddingRequestError, types::EmbeddingRequestRequirements};

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
