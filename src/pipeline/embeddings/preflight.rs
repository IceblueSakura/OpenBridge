//! Resolves and validates Embeddings facts against one immutable operation interface.

use crate::{
    core::{EmbeddingEncoding, OperationKind},
    registry::{ModelExecutionInterface, RuntimeRegistry},
};

use super::super::{error::EmbeddingRequestError, types::EmbeddingRequestRequirements};

/// Resolves and validates one Embeddings request against its immutable typed execution interface.
pub(super) fn preflight_public_model<'a>(
    registry: &'a RuntimeRegistry,
    requirements: &EmbeddingRequestRequirements,
) -> Result<(&'a ModelExecutionInterface, EmbeddingEncoding, u32), EmbeddingRequestError> {
    // Resolve only the selected Public Model and its precompiled Embeddings execution interface.
    let public_model = registry
        .public_model(requirements.public_model())
        .ok_or(EmbeddingRequestError::ModelNotFound)?;
    let interface = public_model
        .execution_interface(OperationKind::EmbeddingsCreate)
        .ok_or_else(|| EmbeddingRequestError::unsupported("model"))?;
    let capabilities = interface
        .embedding_capabilities()
        .ok_or_else(|| EmbeddingRequestError::unsupported("model"))?;

    // Validate the input shape and batch limit against the one fixed interface.
    if !capabilities.supports_input_form(requirements.input_form)
        || requirements.input_count > capabilities.max_inputs()
    {
        return Err(EmbeddingRequestError::unsupported("input"));
    }

    // Validate ownership of each optional standard field before resolving its domain.
    if requirements.user_present && !capabilities.supports_parameter("user") {
        return Err(EmbeddingRequestError::unsupported("user"));
    }
    if requirements.requested_encoding.is_some()
        && !capabilities.supports_parameter("encoding_format")
    {
        return Err(EmbeddingRequestError::unsupported("encoding_format"));
    }
    if requirements.requested_dimensions.is_some() && !capabilities.supports_parameter("dimensions")
    {
        return Err(EmbeddingRequestError::unsupported("dimensions"));
    }

    // Resolve explicit/default encoding and dimensions directly from the same projected contract.
    let encoding = capabilities
        .resolve_encoding(requirements.requested_encoding)
        .ok_or_else(|| EmbeddingRequestError::unsupported("encoding_format"))?;
    let dimensions = capabilities
        .resolve_dimensions(requirements.requested_dimensions)
        .ok_or_else(|| EmbeddingRequestError::unsupported("dimensions"))?;

    // Enforce exact token-array limits only for forms declared locally countable by the interface.
    if capabilities.counts_tokens_locally(requirements.input_form) {
        let token_counts = requirements
            .token_counts
            .as_deref()
            .ok_or_else(|| EmbeddingRequestError::invalid(Some("input")))?;
        if capabilities
            .max_tokens_per_input()
            .is_some_and(|limit| token_counts.iter().any(|count| *count > limit))
        {
            return Err(EmbeddingRequestError::unsupported("input"));
        }
        let total = token_counts
            .iter()
            .try_fold(0_u64, |total, count| total.checked_add(u64::from(*count)))
            .ok_or_else(|| EmbeddingRequestError::unsupported("input"))?;
        if capabilities
            .max_total_tokens()
            .is_some_and(|limit| total > u64::from(limit))
        {
            return Err(EmbeddingRequestError::unsupported("input"));
        }
    }

    // Return resolved response expectations beside the exact interface used for planning.
    Ok((interface, encoding, dimensions))
}
