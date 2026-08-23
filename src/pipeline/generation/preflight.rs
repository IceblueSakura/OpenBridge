//! Validates request facts against one Public Model interface before Route planning.
//!
//! Preflight owns only the fixed downstream contract. It does not inspect candidate-specific
//! capabilities, apply Provider wire mappings, or influence configured Route order.

use crate::{
    core::{ApiProtocol, ImageInputSource, JsonSchemaSupport, StructuredOutputProfile},
    registry::{
        AudioInputInterfaceCapabilities, AudioInterfaceCapabilities,
        AudioOutputInterfaceCapabilities, FileInputSource, ModelExecutionInterface,
        ModelInterfaceCapabilities, ReasoningLevel, RuntimeRegistry, SupportState,
    },
};

use super::super::{
    error::{GenerationCapabilityReason, RequestPlanningError},
    types::{
        AudioInputRequirements, GeneratedAudioMessageShape, InputAudioMessageShape,
        RequestRequirements, RequestedAsrLanguage, RequestedAsrOptions, RequestedAudio,
        RequestedAudioDelivery, RequestedCapabilities, RequestedJsonSchemaStrictness,
        RequestedOutputTokens, RequestedReasoning, RequestedReasoningSummary,
        RequestedStructuredOutput, RequestedVoice,
    },
};

/// Resolves the selected Public Model and validates the request against its compiled protocol interface.
pub(super) fn preflight_public_model<'a>(
    registry: &'a RuntimeRegistry,
    requirements: &RequestRequirements,
) -> Result<(&'a ModelExecutionInterface, Option<ReasoningLevel>), RequestPlanningError> {
    // Resolve the downstream model and its precompiled protocol interface without consulting any Route candidate.
    let public_model = registry
        .public_model(requirements.public_model())
        .ok_or(RequestPlanningError::UnknownModel)?;
    let interface = public_model
        .execution_interface(requirements.protocol().operation())
        .ok_or_else(|| {
            RequestPlanningError::unsupported("model", GenerationCapabilityReason::Protocol)
        })?;

    // Validate every modeled request fact against the single fixed interface contract.
    let normalized_reasoning_level = validate_interface_request(
        &requirements.requested_capabilities,
        requirements.requested_output_tokens,
        requirements.protocol(),
        interface.capabilities(),
        interface.supports_previous_response_id(),
    )?;

    // Reject known parameters outside the same fixed interface after specialized semantic checks.
    if let Some(parameter) = requirements.requested_parameters.iter().find(|parameter| {
        !interface
            .capabilities()
            .supports_parameter(parameter.as_wire_name())
    }) {
        return Err(RequestPlanningError::unsupported(
            parameter.as_wire_name(),
            GenerationCapabilityReason::OrdinaryParameter,
        ));
    }
    Ok((interface, normalized_reasoning_level))
}

/// Returns the most specific fail-closed error from the fixed interface contract.
fn validate_interface_request(
    requested_features: &RequestedCapabilities,
    requested_output_tokens: Option<RequestedOutputTokens>,
    protocol: ApiProtocol,
    interface: &ModelInterfaceCapabilities,
    supports_previous_response_id: bool,
) -> Result<Option<ReasoningLevel>, RequestPlanningError> {
    // Reject invalid reasoning shape/combination before any model capability family.
    if matches!(
        requested_features.reasoning,
        RequestedReasoning::Conflicting
    ) || matches!(
        requested_features.reasoning_summary,
        RequestedReasoningSummary::Invalid
    ) || (matches!(
        requested_features.reasoning_summary,
        RequestedReasoningSummary::Auto
    ) && matches!(
        requested_features.reasoning,
        RequestedReasoning::Level(ReasoningLevel::None)
    )) {
        return Err(RequestPlanningError::InvalidReasoningConfiguration {
            param: reasoning_param(protocol),
        });
    }

    // Validate delivery before field families so the public first-error order stays stable.
    if requested_features.streaming && !interface.supports_streaming() {
        return Err(RequestPlanningError::unsupported(
            "stream",
            GenerationCapabilityReason::Streaming,
        ));
    }
    if !requested_features.streaming && !interface.supports_non_streaming() {
        return Err(RequestPlanningError::unsupported(
            "stream",
            GenerationCapabilityReason::NonStreaming,
        ));
    }

    // Validate tools in an explicit order independent of sets, JSON keys, or candidate order.
    if requested_features.unmodeled_tools {
        return Err(RequestPlanningError::unsupported(
            "tools",
            GenerationCapabilityReason::Tools,
        ));
    }
    if requested_features.function_tools && !interface.supports_function_tools() {
        return Err(RequestPlanningError::unsupported(
            "tools",
            GenerationCapabilityReason::Tools,
        ));
    }
    if requested_features.unknown_tool_choice
        || requested_features
            .function_tool_choice
            .is_some_and(|mode| !interface.supports_tool_choice(mode))
    {
        return Err(RequestPlanningError::unsupported(
            "tool_choice",
            GenerationCapabilityReason::ToolChoice,
        ));
    }
    if requested_features.parallel_tool_calls && !interface.supports_parallel_tool_calls() {
        return Err(RequestPlanningError::unsupported(
            "parallel_tool_calls",
            GenerationCapabilityReason::ParallelToolCalls,
        ));
    }
    if requested_features.function_tool_strict_schema && !interface.supports_strict_tool_schema() {
        return Err(RequestPlanningError::unsupported(
            "tools",
            GenerationCapabilityReason::StrictToolSchema,
        ));
    }

    // Validate Structured Output after tools using the source protocol's standard owner field.
    if !supports_requested_structured_output(
        requested_features.structured_output,
        interface.structured_outputs(),
    ) {
        return Err(RequestPlanningError::unsupported(
            structured_output_param(protocol),
            GenerationCapabilityReason::StructuredOutput,
        ));
    }

    // Validate state before include, multimodal, limits, and reasoning.
    if requested_features.previous_response_id && !supports_previous_response_id {
        return Err(RequestPlanningError::unsupported(
            "previous_response_id",
            GenerationCapabilityReason::PreviousResponse,
        ));
    }
    if requested_features.background && !interface.supports_background() {
        return Err(RequestPlanningError::unsupported(
            "background",
            GenerationCapabilityReason::Background,
        ));
    }
    if requested_features
        .response_includes
        .iter()
        .any(|include| !interface.supports_response_include(*include))
    {
        return Err(RequestPlanningError::unsupported(
            "include",
            GenerationCapabilityReason::ResponseInclude,
        ));
    }

    // Validate every frozen image source, format, detail, and local limit against one fixed profile.
    if let Some(requested) = requested_features.image_input.as_ref() {
        let input_param = generation_input_param(protocol);
        let image = interface.image_input().ok_or_else(|| {
            RequestPlanningError::unsupported(input_param, GenerationCapabilityReason::ImageInput)
        })?;
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
            return Err(RequestPlanningError::unsupported(
                input_param,
                GenerationCapabilityReason::ImageInput,
            ));
        }

        // Keep locally countable size failures on the same top-level multimodal owner.
        let exceeds_limit = requested.part_count > image.max_parts()
            || (requested.sources.contains(&ImageInputSource::RemoteUrl)
                && exceeds_image_limit(requested.max_url_length, image.max_url_length()))
            || (requested.sources.contains(&ImageInputSource::DataUrl)
                && (exceeds_image_limit(
                    requested.max_inline_encoded_bytes,
                    image.max_inline_encoded_bytes(),
                ) || exceeds_image_limit(
                    requested.max_inline_decoded_bytes,
                    image.max_inline_decoded_bytes(),
                ) || exceeds_image_limit(
                    requested.total_inline_encoded_bytes,
                    image.max_total_inline_encoded_bytes(),
                ) || exceeds_image_limit(
                    requested.total_inline_decoded_bytes,
                    image.max_total_inline_decoded_bytes(),
                )));
        if exceeds_limit {
            return Err(RequestPlanningError::unsupported(
                input_param,
                GenerationCapabilityReason::ImageInput,
            ));
        }
    }

    // Validate file source, encoding, media type, filename, and byte budgets before Route planning.
    if let Some(requested) = requested_features.file_input.as_ref() {
        let input_param = generation_input_param(protocol);
        let file = interface.file_input().ok_or_else(|| {
            RequestPlanningError::unsupported(input_param, GenerationCapabilityReason::FileInput)
        })?;
        if requested
            .sources
            .iter()
            .any(|source| !file.supports_source(*source))
            || requested
                .encodings
                .iter()
                .any(|encoding| !file.supports_encoding(*encoding))
            || requested
                .media_types
                .iter()
                .any(|media_type| !file.supports_media_type(*media_type))
            || requested
                .details
                .iter()
                .any(|detail| !file.supports_detail(*detail))
        {
            return Err(RequestPlanningError::unsupported(
                input_param,
                GenerationCapabilityReason::FileInput,
            ));
        }
        let exceeds_limit = requested.part_count > file.max_parts()
            || requested.max_filename_length > file.max_filename_length()
            || (requested.sources.contains(&FileInputSource::RemoteUrl)
                && exceeds_image_limit(requested.max_url_length, file.max_url_length()))
            || (requested.sources.contains(&FileInputSource::InlineData)
                && (exceeds_image_limit(
                    requested.max_inline_encoded_bytes,
                    file.max_inline_encoded_bytes(),
                ) || exceeds_image_limit(
                    requested.max_inline_decoded_bytes,
                    file.max_inline_decoded_bytes(),
                ) || exceeds_image_limit(
                    requested.total_inline_encoded_bytes,
                    file.max_total_inline_encoded_bytes(),
                ) || exceeds_image_limit(
                    requested.total_inline_decoded_bytes,
                    file.max_total_inline_decoded_bytes(),
                )));
        if exceeds_limit {
            return Err(RequestPlanningError::unsupported(
                input_param,
                GenerationCapabilityReason::FileInput,
            ));
        }
    }

    // Interpret one frozen audio wire shape only after resolving the fixed interface profile.
    validate_audio_request(
        requested_features.audio.as_ref(),
        interface.audio(),
        requested_features.streaming,
        protocol,
    )?;

    // Enforce the fixed output limit while retaining the deterministic source field.
    if let (Some(requested), Some(limit)) = (requested_output_tokens, interface.max_output_tokens())
        && requested.value > u64::from(limit)
    {
        return Err(RequestPlanningError::unsupported(
            requested.param,
            GenerationCapabilityReason::OutputLimit,
        ));
    }

    // Resolve reasoning against the fixed Public Model policy without applying Provider mappings.
    let normalized_reasoning_level = match requested_features.reasoning {
        RequestedReasoning::None => None,
        RequestedReasoning::Unspecified
            if interface.reasoning_support() != SupportState::Supported =>
        {
            return Err(RequestPlanningError::unsupported(
                reasoning_param(protocol),
                GenerationCapabilityReason::Reasoning,
            ));
        }
        RequestedReasoning::Level(level) => {
            let effective = interface.resolve_reasoning_level(level).ok_or_else(|| {
                RequestPlanningError::unsupported(
                    reasoning_param(protocol),
                    GenerationCapabilityReason::ReasoningLevel,
                )
            })?;
            (effective != level).then_some(effective)
        }
        RequestedReasoning::UnknownLevel => {
            return Err(RequestPlanningError::unsupported(
                reasoning_param(protocol),
                GenerationCapabilityReason::ReasoningLevel,
            ));
        }
        RequestedReasoning::Conflicting => {
            unreachable!("invalid reasoning configuration returned before capability checks")
        }
        RequestedReasoning::Unspecified => None,
    };

    // Validate summary syntax independently from effort and reject a summary request when reasoning is explicitly disabled.
    match requested_features.reasoning_summary {
        RequestedReasoningSummary::Invalid => {
            unreachable!("invalid reasoning summary returned before capability checks")
        }
        RequestedReasoningSummary::Auto
            if matches!(
                requested_features.reasoning,
                RequestedReasoning::Level(ReasoningLevel::None)
            ) =>
        {
            unreachable!("invalid reasoning summary combination returned before capability checks")
        }
        RequestedReasoningSummary::Absent
        | RequestedReasoningSummary::Disabled
        | RequestedReasoningSummary::Auto => {}
    }
    Ok(normalized_reasoning_level)
}

/// Returns the source protocol's top-level multimodal input owner.
const fn generation_input_param(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::ChatCompletions => "messages",
        ApiProtocol::Responses => "input",
    }
}

/// Returns the source protocol's Structured Output owner.
const fn structured_output_param(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::ChatCompletions => "response_format",
        ApiProtocol::Responses => "text",
    }
}

/// Returns the source protocol's reasoning owner.
const fn reasoning_param(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::ChatCompletions => "reasoning_effort",
        ApiProtocol::Responses => "reasoning",
    }
}

/// Returns whether one closed interface profile accepts the complete structured-output request.
fn supports_requested_structured_output(
    requested: RequestedStructuredOutput,
    supported: Option<StructuredOutputProfile>,
) -> bool {
    use JsonSchemaSupport::StrictSupported;
    use RequestedJsonSchemaStrictness::{NonStrict, Strict};
    use RequestedStructuredOutput::{JsonObject, JsonSchema, Unconstrained};
    use StructuredOutputProfile::{
        JsonObject as SupportsJsonObject, JsonObjectAndJsonSchema, JsonSchema as SupportsJsonSchema,
    };

    matches!(
        (requested, supported),
        (Unconstrained, _)
            | (
                JsonObject,
                Some(SupportsJsonObject | JsonObjectAndJsonSchema(_))
            )
            | (
                JsonSchema(NonStrict),
                Some(SupportsJsonSchema(_) | JsonObjectAndJsonSchema(_))
            )
            | (
                JsonSchema(Strict),
                Some(
                    SupportsJsonSchema(StrictSupported) | JsonObjectAndJsonSchema(StrictSupported)
                )
            )
    )
}

/// Returns whether one request measurement exceeds a source-specific owned image limit.
fn exceeds_image_limit(requested: u32, limit: Option<u32>) -> bool {
    match limit {
        Some(limit) => requested > limit,
        None => true,
    }
}

/// Validates one analyzed audio input against a fixed public source, format, and size profile.
fn validate_audio_input(
    requested: &AudioInputRequirements,
    profile: &AudioInputInterfaceCapabilities,
    param: &'static str,
) -> Result<(), RequestPlanningError> {
    // Reject unsupported source/format pairs before evaluating their source-owned limits.
    if requested.sources.iter().any(|(source, requirements)| {
        !profile.supports_source(*source)
            || requirements
                .formats
                .iter()
                .any(|format| !profile.supports_format(*source, *format))
    }) {
        return Err(RequestPlanningError::unsupported(
            param,
            GenerationCapabilityReason::AudioInput,
        ));
    }

    // Apply global cardinality and then the limits owned by each requested source.
    if requested.part_count > profile.max_parts() {
        return Err(RequestPlanningError::unsupported(
            param,
            GenerationCapabilityReason::AudioInput,
        ));
    }
    for (source, requirements) in &requested.sources {
        let exceeds = match source {
            crate::core::AudioInputSource::RemoteUrl => {
                requirements.max_url_length
                    > profile
                        .max_url_length()
                        .expect("supported remote audio source has a URL limit")
            }
            crate::core::AudioInputSource::DataUrl | crate::core::AudioInputSource::Base64 => {
                requirements.max_inline_encoded_bytes
                    > profile
                        .max_inline_encoded_bytes(*source)
                        .expect("supported inline audio source has encoded limits")
                    || requirements.max_inline_decoded_bytes
                        > profile
                            .max_inline_decoded_bytes(*source)
                            .expect("supported inline audio source has decoded limits")
                    || requirements.total_inline_encoded_bytes
                        > profile
                            .max_total_inline_encoded_bytes(*source)
                            .expect("supported inline audio source has total encoded limits")
                    || requested.total_inline_encoded_bytes
                        > profile
                            .max_total_inline_encoded_bytes(*source)
                            .expect("supported inline audio source has aggregate encoded limits")
                    || requirements.total_inline_decoded_bytes
                        > profile
                            .max_total_inline_decoded_bytes(*source)
                            .expect("supported inline audio source has total decoded limits")
                    || requested.total_inline_decoded_bytes
                        > profile
                            .max_total_inline_decoded_bytes(*source)
                            .expect("supported inline audio source has aggregate decoded limits")
            }
        };
        if exceeds {
            return Err(RequestPlanningError::unsupported(
                param,
                GenerationCapabilityReason::AudioInput,
            ));
        }
    }
    Ok(())
}

/// Matches one structural request union against one fixed executable audio profile.
fn validate_audio_request(
    requested: Option<&RequestedAudio>,
    profile: Option<&AudioInterfaceCapabilities>,
    streaming: bool,
    protocol: ApiProtocol,
) -> Result<(), RequestPlanningError> {
    let input_param = generation_input_param(protocol);
    match (requested, profile) {
        (None, None | Some(AudioInterfaceCapabilities::AudioUnderstanding { .. })) => Ok(()),
        (None, Some(_)) | (Some(_), None) => Err(RequestPlanningError::unsupported(
            input_param,
            GenerationCapabilityReason::AudioInput,
        )),
        (
            Some(RequestedAudio::Input {
                resources,
                asr_options,
                ..
            }),
            Some(AudioInterfaceCapabilities::AudioUnderstanding { input }),
        ) => {
            validate_audio_input(resources, input, input_param)?;
            if !matches!(asr_options, RequestedAsrOptions::Absent) {
                return Err(RequestPlanningError::unsupported(
                    "asr_options",
                    GenerationCapabilityReason::AudioInput,
                ));
            }
            Ok(())
        }
        (
            Some(RequestedAudio::Input {
                resources,
                message_shape,
                asr_options,
            }),
            Some(AudioInterfaceCapabilities::SpeechRecognition { input, languages }),
        ) => {
            validate_audio_input(resources, input, "messages")?;
            if resources.part_count != 1
                || *message_shape != InputAudioMessageShape::SingleUserAudioOnly
            {
                return Err(RequestPlanningError::unsupported(
                    "messages",
                    GenerationCapabilityReason::AudioInput,
                ));
            }
            validate_asr_options(*asr_options, languages)
        }
        (
            Some(RequestedAudio::Generated {
                delivery,
                message_shape,
                voice,
            }),
            Some(AudioInterfaceCapabilities::SpeechSynthesis { output }),
        ) => {
            validate_audio_output(*delivery, output, streaming)?;
            if !matches!(
                message_shape,
                GeneratedAudioMessageShape::AssistantTextOnly
                    | GeneratedAudioMessageShape::UserTextThenAssistantText
            ) {
                return Err(RequestPlanningError::unsupported(
                    "messages",
                    GenerationCapabilityReason::AudioOutput,
                ));
            }
            match voice {
                RequestedVoice::Unspecified => Ok(()),
                RequestedVoice::Preset(voice) if output.supports_voice(voice) => Ok(()),
                RequestedVoice::Preset(_) | RequestedVoice::ReferenceVoice(_) => {
                    Err(RequestPlanningError::unsupported(
                        "audio",
                        GenerationCapabilityReason::AudioOutput,
                    ))
                }
            }
        }
        (
            Some(RequestedAudio::Generated {
                delivery,
                message_shape,
                voice,
            }),
            Some(AudioInterfaceCapabilities::VoiceDesign { output }),
        ) => {
            validate_audio_output(*delivery, output, streaming)?;
            if *message_shape != GeneratedAudioMessageShape::UserTextThenAssistantText
                || !matches!(voice, RequestedVoice::Unspecified)
            {
                return Err(RequestPlanningError::unsupported(
                    "messages",
                    GenerationCapabilityReason::AudioOutput,
                ));
            }
            Ok(())
        }
        (
            Some(RequestedAudio::Generated {
                delivery,
                message_shape,
                voice,
            }),
            Some(AudioInterfaceCapabilities::VoiceClone {
                conditioning,
                output,
            }),
        ) => {
            validate_audio_output(*delivery, output, streaming)?;
            let RequestedVoice::ReferenceVoice(reference) = voice else {
                return Err(RequestPlanningError::unsupported(
                    "audio",
                    GenerationCapabilityReason::AudioInput,
                ));
            };
            validate_audio_input(reference, conditioning, "audio")?;
            if *message_shape != GeneratedAudioMessageShape::AssistantTextOnly {
                return Err(RequestPlanningError::unsupported(
                    "messages",
                    GenerationCapabilityReason::AudioOutput,
                ));
            }
            Ok(())
        }
        (Some(RequestedAudio::Input { .. }), Some(_))
        | (Some(RequestedAudio::Generated { .. }), Some(_)) => Err(
            RequestPlanningError::unsupported(input_param, GenerationCapabilityReason::AudioInput),
        ),
    }
}

/// Validates optional ASR controls against the exact language set in the executable profile.
fn validate_asr_options(
    requested: RequestedAsrOptions,
    languages: &[crate::core::AsrLanguage],
) -> Result<(), RequestPlanningError> {
    match requested {
        RequestedAsrOptions::Absent | RequestedAsrOptions::Present { language: None } => Ok(()),
        RequestedAsrOptions::Present {
            language: Some(RequestedAsrLanguage::Known(language)),
        } if languages.contains(&language) => Ok(()),
        RequestedAsrOptions::Present {
            language: Some(RequestedAsrLanguage::Known(_) | RequestedAsrLanguage::Unsupported),
        } => Err(RequestPlanningError::unsupported(
            "asr_options",
            GenerationCapabilityReason::AudioInput,
        )),
    }
}

/// Validates generated-audio delivery format and the positive bounded response profile.
fn validate_audio_output(
    requested: RequestedAudioDelivery,
    profile: &AudioOutputInterfaceCapabilities,
    streaming: bool,
) -> Result<(), RequestPlanningError> {
    if !profile.supports_format(requested.format, streaming)
        || (streaming && profile.max_stream_decoded_bytes() == 0)
        || (!streaming
            && (profile.max_inline_encoded_bytes() == 0 || profile.max_inline_decoded_bytes() == 0))
    {
        return Err(RequestPlanningError::unsupported(
            "audio",
            GenerationCapabilityReason::AudioOutput,
        ));
    }
    Ok(())
}
