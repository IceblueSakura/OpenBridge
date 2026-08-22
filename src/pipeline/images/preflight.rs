//! Resolves and validates Images Generations facts against one immutable operation interface.

use crate::{
    core::{ImagesOutputFormat, ImagesResponseFormat, OperationKind},
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

    // Structurally valid standard fields remain known even when this qwen profile cannot execute them.
    if let Some(field) = requirements.unsupported_standard_fields.first() {
        return Err(ImagesRequestError::unsupported(field.parameter()));
    }

    // DashScope extension fields require one explicit model-bound extension profile.
    match capabilities.dashscope_extensions() {
        None => {
            if let Some(parameter) = requirements.dashscope.first_present_parameter() {
                return Err(ImagesRequestError::unsupported(parameter));
            }
        }
        Some(extensions) => {
            if requirements
                .dashscope
                .prompt_extend_mode
                .is_some_and(|mode| !extensions.prompt_extend_modes.contains(&mode))
            {
                return Err(ImagesRequestError::unsupported("prompt_extend_mode"));
            }
            if requirements.dashscope.negative_prompt_present && !extensions.negative_prompt {
                return Err(ImagesRequestError::unsupported("negative_prompt"));
            }
            if requirements
                .dashscope
                .seed
                .is_some_and(|seed| seed > extensions.maximum_seed)
            {
                return Err(ImagesRequestError::unsupported("seed"));
            }
        }
    }

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
    if let Some(output_format) = requirements.requested_output_format
        && (!capabilities.supports_parameter("output_format")
            || output_format != ImagesOutputFormat::Png)
    {
        return Err(ImagesRequestError::unsupported("output_format"));
    }
    if requirements.requested_stream == Some(true) {
        return Err(ImagesRequestError::unsupported("stream"));
    }
    if requirements.user_present && !capabilities.supports_parameter("user") {
        return Err(ImagesRequestError::unsupported("user"));
    }

    // Resolve explicit/default values directly from the same projected contract.
    let outputs = capabilities
        .resolve_outputs(requirements.requested_outputs)
        .ok_or_else(|| ImagesRequestError::unsupported("n"))?;
    if let Some(ImagesRequestedSize::Exact { width, height }) = requirements.requested_size
        && !capabilities.supports_size(width, height)
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
