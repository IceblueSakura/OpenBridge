//! Validates request facts against one Public Model interface before Route planning.
//!
//! Preflight owns only the fixed downstream contract. It does not inspect candidate-specific
//! capabilities, apply Provider wire mappings, or influence configured Route order.

use crate::{
    core::{AudioTask, EmbeddingEncoding, OperationKind},
    registry::{
        ModelExecutionInterface, ModelInterfaceCapabilities, ReasoningLevel, RuntimeRegistry,
        SupportState,
    },
};

use super::{
    error::{EmbeddingRequestError, RequestPlanningError},
    types::{
        EmbeddingRequestRequirements, RequestRequirements, RequestedCapabilities,
        RequestedReasoning,
    },
};

/// Resolves the selected Public Model and validates the request against its compiled protocol interface.
pub(super) fn preflight_public_model<'a>(
    registry: &'a RuntimeRegistry,
    requirements: &RequestRequirements,
) -> Result<&'a ModelExecutionInterface, RequestPlanningError> {
    // Resolve the downstream model and its precompiled protocol interface without consulting any Route candidate.
    let public_model = registry
        .public_model(requirements.public_model())
        .ok_or(RequestPlanningError::UnknownModel)?;
    let interface = public_model
        .execution_interface(requirements.protocol().operation())
        .ok_or(RequestPlanningError::UnsupportedProtocol)?;

    // Validate every modeled request fact against the single fixed interface contract.
    validate_interface_request(
        &requirements.requested_capabilities,
        requirements.requested_output_tokens,
        interface.capabilities(),
    )?;
    Ok(interface)
}

/// Resolves and validates one Embeddings request against its immutable typed execution interface.
pub(super) fn preflight_embedding_public_model<'a>(
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

/// Returns the most specific fail-closed error from the fixed interface contract.
fn validate_interface_request(
    requested_features: &RequestedCapabilities,
    requested_output_tokens: Option<u64>,
    interface: &ModelInterfaceCapabilities,
) -> Result<(), RequestPlanningError> {
    // Validate shared generation and state capabilities before any egress preparation.
    if requested_features.unmodeled_tools {
        return Err(RequestPlanningError::UnsupportedCapabilities);
    }
    if requested_features.streaming && !interface.supports_streaming() {
        return Err(RequestPlanningError::StreamingUnsupported);
    }
    if (requested_features.function_calling && !interface.supports_function_calling())
        || (requested_features.parallel_tool_calls && !interface.supports_parallel_tool_calls())
        || (requested_features.structured_outputs && !interface.supports_structured_outputs())
        || (requested_features.store && !interface.supports_store())
        || (requested_features.previous_response_id && !interface.supports_previous_response_id())
        || (requested_features.background && !interface.supports_background())
    {
        return Err(RequestPlanningError::UnsupportedCapabilities);
    }

    // Validate every frozen image source, format, detail, and local limit against one fixed profile.
    if let Some(requested) = requested_features.image_input.as_ref() {
        let image = interface
            .image_input()
            .ok_or(RequestPlanningError::UnsupportedCapabilities)?;
        let unsupported = requested.unsupported_media_type
            || requested
                .sources
                .iter()
                .any(|source| !image.supports_source(*source))
            || requested
                .media_types
                .iter()
                .any(|media_type| !image.supports_media_type(*media_type))
            || requested
                .details
                .iter()
                .any(|detail| !image.supports_detail(*detail));
        if unsupported {
            return Err(RequestPlanningError::UnsupportedCapabilities);
        }

        // Keep locally countable size failures distinct from unsupported source semantics.
        let exceeds_limit = requested.part_count > image.max_parts()
            || requested.max_url_length > image.max_url_length()
            || requested.max_inline_encoded_bytes > image.max_inline_encoded_bytes()
            || requested.max_inline_decoded_bytes > image.max_inline_decoded_bytes()
            || requested.total_inline_encoded_bytes > image.max_total_inline_encoded_bytes()
            || requested.total_inline_decoded_bytes > image.max_total_inline_decoded_bytes();
        if exceeds_limit {
            return Err(RequestPlanningError::MultimodalInputLimitExceeded);
        }
    }

    // Validate task-specific audio inputs, conditioning resources, and bounded inline sizes.
    if let Some(requested) = requested_features.audio_input.as_ref() {
        let audio = interface
            .audio_input()
            .ok_or(RequestPlanningError::UnsupportedCapabilities)?;
        validate_audio_input(requested, audio)?;
    }
    if let Some(requested) = requested_features.voice_conditioning.as_ref() {
        let voice = interface
            .voice_conditioning()
            .ok_or(RequestPlanningError::UnsupportedCapabilities)?;
        validate_audio_input(requested, voice)?;
    }
    if let Some(requested) = requested_features.audio_output.as_ref() {
        let audio = interface
            .audio_output()
            .ok_or(RequestPlanningError::UnsupportedCapabilities)?;
        if !audio.supports_format(requested.format, requested_features.streaming) {
            return Err(RequestPlanningError::UnsupportedCapabilities);
        }
        if let Some(voice) = requested.voice.as_deref()
            && !audio.supports_voice(voice)
        {
            return Err(RequestPlanningError::UnsupportedCapabilities);
        }
        if requested_features.streaming {
            if audio.max_stream_decoded_bytes() == 0 {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            }
        } else if audio.max_inline_encoded_bytes() == 0 || audio.max_inline_decoded_bytes() == 0 {
            return Err(RequestPlanningError::UnsupportedCapabilities);
        }
    }

    // Enforce the fixed task identity and message shape after generic audio facts are validated.
    validate_audio_task(requested_features, interface.audio_task())?;

    // Enforce the fixed output limit when the request carries an explicit value.
    if interface.max_output_tokens().is_some_and(|limit| {
        requested_output_tokens.is_some_and(|requested| requested > u64::from(limit))
    }) {
        return Err(RequestPlanningError::OutputLimitExceeded);
    }

    // Validate reasoning support and the fixed public level set without applying Provider mappings.
    match requested_features.reasoning {
        RequestedReasoning::None | RequestedReasoning::Level(ReasoningLevel::None) => {}
        RequestedReasoning::Unspecified
            if interface.reasoning_support() != SupportState::Supported =>
        {
            return Err(RequestPlanningError::ReasoningUnsupported);
        }
        RequestedReasoning::Level(level)
            if interface.reasoning_support() != SupportState::Supported
                || !interface.reasoning_levels().contains(&level) =>
        {
            return Err(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::UnknownLevel => {
            return Err(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::Conflicting => {
            return Err(RequestPlanningError::InvalidReasoningConfiguration);
        }
        RequestedReasoning::Unspecified | RequestedReasoning::Level(_) => {}
    }
    Ok(())
}

/// Validates one analyzed audio input against a fixed public source, format, and size profile.
fn validate_audio_input(
    requested: &super::types::AudioInputRequirements,
    profile: &crate::registry::AudioInputInterfaceCapabilities,
) -> Result<(), RequestPlanningError> {
    if requested.part_count > profile.max_parts()
        || requested.max_url_length > profile.max_url_length()
        || requested.max_inline_encoded_bytes > profile.max_inline_encoded_bytes()
        || requested.max_inline_decoded_bytes > profile.max_inline_decoded_bytes()
        || requested.total_inline_encoded_bytes > profile.max_total_inline_encoded_bytes()
        || requested.total_inline_decoded_bytes > profile.max_total_inline_decoded_bytes()
    {
        return Err(RequestPlanningError::MultimodalInputLimitExceeded);
    }
    if requested
        .sources
        .iter()
        .any(|source| !profile.supports_source(*source))
        || requested
            .formats
            .iter()
            .any(|format| !profile.supports_format(*format))
    {
        return Err(RequestPlanningError::UnsupportedCapabilities);
    }
    Ok(())
}

/// Enforces task-specific required inputs and target-text semantics without selecting a Route.
fn validate_audio_task(
    requested: &RequestedCapabilities,
    task: Option<AudioTask>,
) -> Result<(), RequestPlanningError> {
    let Some(task) = task else {
        if requested.audio_input.is_some()
            || requested.voice_conditioning.is_some()
            || requested.audio_output.is_some()
            || requested.asr_options_present
        {
            return Err(RequestPlanningError::UnsupportedCapabilities);
        }
        return Ok(());
    };
    match task {
        AudioTask::Asr => {
            let Some(input) = requested.audio_input.as_ref() else {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            };
            if input.part_count != 1
                || input.text_part_count != 0
                || requested.audio_output.is_some()
            {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            }
            if requested
                .asr_language
                .as_deref()
                .is_some_and(|language| !matches!(language, "auto" | "zh" | "en"))
            {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            }
            if !requested.asr_options_present && requested.asr_language.is_some() {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
        }
        AudioTask::Tts => {
            let Some(output) = requested.audio_output.as_ref() else {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            };
            if output.assistant_text_count != 1
                || requested.audio_input.is_some()
                || requested.voice_conditioning.is_some()
                || requested.asr_options_present
            {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            }
        }
        AudioTask::VoiceDesign => {
            let Some(output) = requested.audio_output.as_ref() else {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            };
            if output.assistant_text_count != 1
                || !output.voice_description
                || requested.audio_input.is_some()
                || requested.voice_conditioning.is_some()
                || requested.asr_options_present
            {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            }
        }
        AudioTask::VoiceClone => {
            let Some(output) = requested.audio_output.as_ref() else {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            };
            if output.assistant_text_count != 1
                || requested.voice_conditioning.is_none()
                || requested.audio_input.is_some()
                || requested.asr_options_present
            {
                return Err(RequestPlanningError::UnsupportedCapabilities);
            }
        }
        AudioTask::AudioUnderstanding | AudioTask::Any => {}
    }
    Ok(())
}
