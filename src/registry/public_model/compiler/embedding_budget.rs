//! Checked Embeddings response-budget narrowing.
//!
//! The compiler derives a safe maximum batch size from the largest permitted vector representation
//! and both upstream and downstream JSON envelopes before the interface can become executable.

use crate::{
    core::{EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingsCapabilities, OperationKind},
    registry::RegistryError,
};

use super::PrecompiledRouteCandidate;

/// Narrows the single Embeddings candidate using checked worst-case JSON serialization bounds.
pub(super) fn constrain_embedding_response_budget(
    public_model: &str,
    response_budget: usize,
    candidates: &mut [PrecompiledRouteCandidate],
) -> Result<(), RegistryError> {
    // Locate the one compiler-approved Embeddings candidate without affecting generation interfaces.
    let Some(candidate) = candidates.iter_mut().find(|candidate| {
        candidate.execution.downstream_operation() == OperationKind::EmbeddingsCreate
    }) else {
        return Ok(());
    };
    let Some(mut capabilities) = candidate.contribution.embedding_capabilities else {
        return Err(RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        });
    };

    // Compute the largest public dimension and the worst permitted vector encoding.
    let maximum_dimensions = maximum_embedding_dimensions(capabilities)
        .and_then(|dimensions| usize::try_from(dimensions).ok())
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;
    let vector_bytes = worst_case_embedding_vector_bytes(capabilities, maximum_dimensions)
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;

    // Bound the raw upstream and projected downstream envelopes, then derive the safe batch count.
    let envelope_bytes = embedding_response_envelope_bytes(candidate.execution.upstream_model())
        .and_then(|upstream| {
            embedding_response_envelope_bytes(public_model)
                .map(|downstream| upstream.max(downstream))
        })
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;
    let item_bytes = embedding_response_item_bytes(vector_bytes).ok_or_else(|| {
        RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        }
    })?;
    let available_with_first_separator = response_budget
        .checked_sub(envelope_bytes)
        .and_then(|available| available.checked_add(1))
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;
    let bytes_per_additional_item = item_bytes.checked_add(1).ok_or_else(|| {
        RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        }
    })?;
    let budget_max_inputs = available_with_first_separator / bytes_per_additional_item;
    let budget_max_inputs = u32::try_from(budget_max_inputs).unwrap_or(u32::MAX);
    if budget_max_inputs == 0 {
        return Err(RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        });
    }

    // Publish and preflight only the minimum of Provider and runtime response-budget limits.
    capabilities.max_inputs = capabilities.max_inputs.min(budget_max_inputs);
    candidate.contribution.embedding_capabilities = Some(capabilities);
    Ok(())
}

/// Returns the largest dimension a client can receive from an Embeddings interface.
fn maximum_embedding_dimensions(capabilities: EmbeddingsCapabilities) -> Option<u32> {
    match capabilities.allowed_dimensions {
        Some(EmbeddingDimensionDomain::Range { maximum, .. }) => Some(maximum),
        Some(EmbeddingDimensionDomain::Values { values }) => values.last().copied(),
        None => Some(capabilities.default_dimensions),
    }
}

/// Returns the worst JSON byte length of one vector among all permitted encodings.
fn worst_case_embedding_vector_bytes(
    capabilities: EmbeddingsCapabilities,
    dimensions: usize,
) -> Option<usize> {
    // Include the default plus every explicitly requestable encoding without double-counting.
    let mut encodings = vec![capabilities.default_encoding];
    if let Some(allowed) = capabilities.allowed_encodings {
        for encoding in allowed {
            if !encodings.contains(encoding) {
                encodings.push(*encoding);
            }
        }
    }
    encodings
        .into_iter()
        .map(|encoding| embedding_vector_bytes(encoding, dimensions))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max()
}

/// Returns a checked upper bound for one normalized vector JSON value.
fn embedding_vector_bytes(encoding: EmbeddingEncoding, dimensions: usize) -> Option<usize> {
    const MAX_NORMALIZED_JSON_NUMBER_BYTES: usize = 32;
    match encoding {
        EmbeddingEncoding::Float => dimensions
            .checked_mul(MAX_NORMALIZED_JSON_NUMBER_BYTES)
            .and_then(|numbers| numbers.checked_add(dimensions.saturating_sub(1)))
            .and_then(|contents| contents.checked_add(2)),
        EmbeddingEncoding::Base64 => dimensions
            .checked_mul(std::mem::size_of::<f32>())
            .and_then(|bytes| bytes.checked_add(2))
            .map(|rounded| rounded / 3)
            .and_then(|groups| groups.checked_mul(4))
            .and_then(|characters| characters.checked_add(2)),
    }
}

/// Returns the fixed JSON bytes for one data item around its bounded vector and index.
fn embedding_response_item_bytes(vector_bytes: usize) -> Option<usize> {
    const ITEM_PREFIX: &str = r#"{"object":"embedding","embedding":"#;
    const ITEM_SUFFIX_AND_MAX_INDEX: &str = r#", "index":4294967295}"#;
    ITEM_PREFIX
        .len()
        .checked_add(vector_bytes)
        .and_then(|bytes| bytes.checked_add(ITEM_SUFFIX_AND_MAX_INDEX.len()))
}

/// Returns a fixed top-level response envelope using worst-case usage counters and model escaping.
fn embedding_response_envelope_bytes(model: &str) -> Option<usize> {
    // Serialize the trusted model string so quotes and non-ASCII content use the real JSON bound.
    let model = serde_json::to_string(model).ok()?;
    let envelope = format!(
        r#"{{"object":"list","data":[],"model":{model},"usage":{{"prompt_tokens":18446744073709551615,"total_tokens":18446744073709551615}}}}"#
    );
    Some(envelope.len())
}
