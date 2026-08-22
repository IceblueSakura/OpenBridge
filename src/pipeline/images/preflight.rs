//! Resolves and validates Images Generations facts against one immutable operation interface.

use crate::{
    core::{ImagesResponseFormat, OperationKind},
    registry::{ModelExecutionInterface, RuntimeRegistry},
};

use super::super::{
    error::ImagesRequestError,
    types::{ImagesRequestRequirements, ImagesRequestedSize},
};

/// Resolved effective request expectations after fixed-interface preflight.
pub(super) struct ImagesPreflight {
    pub(super) outputs: u32,
    pub(super) size: Option<ImagesRequestedSize>,
    pub(super) response_format: ImagesResponseFormat,
}

/// Resolves and validates one Images request against its immutable typed execution interface.
pub(super) fn preflight_public_model<'a>(
    registry: &'a RuntimeRegistry,
    requirements: &ImagesRequestRequirements,
) -> Result<(&'a ModelExecutionInterface, ImagesPreflight), ImagesRequestError> {
    // Resolve only the selected Public Model and its precompiled Images execution interface.
    let public_model = registry
        .public_model(requirements.public_model())
        .ok_or(ImagesRequestError::ModelNotFound)?;
    let interface = public_model
        .execution_interface(OperationKind::ImagesGenerations)
        .ok_or_else(|| ImagesRequestError::unsupported("model"))?;
    let capabilities = interface
        .images_capabilities()
        .ok_or_else(|| ImagesRequestError::unsupported("model"))?;

    // Validate ownership of each optional standard field before resolving its domain.
    if requirements.requested_outputs.is_some() && !capabilities.supports_parameter("n") {
        return Err(ImagesRequestError::unsupported("n"));
    }
    if requirements.requested_size.is_some() && !capabilities.supports_parameter("size") {
        return Err(ImagesRequestError::unsupported("size"));
    }
    if requirements.requested_response_format.is_some()
        && !capabilities.supports_parameter("response_format")
    {
        return Err(ImagesRequestError::unsupported("response_format"));
    }
    if requirements.user_present && !capabilities.supports_parameter("user") {
        return Err(ImagesRequestError::unsupported("user"));
    }

    // Resolve explicit/default values directly from the same projected contract.
    let outputs = capabilities
        .resolve_outputs(requirements.requested_outputs)
        .ok_or_else(|| ImagesRequestError::unsupported("n"))?;
    if let Some(requested) = requirements.requested_size
        && !capabilities.supports_size(requested.width(), requested.height())
    {
        return Err(ImagesRequestError::unsupported("size"));
    }
    let response_format = capabilities
        .resolve_response_format(requirements.requested_response_format)
        .ok_or_else(|| ImagesRequestError::unsupported("response_format"))?;

    // Return resolved response expectations beside the exact interface used for planning.
    Ok((
        interface,
        ImagesPreflight {
            outputs,
            size: requirements.requested_size,
            response_format,
        },
    ))
}
