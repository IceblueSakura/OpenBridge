//! Static Xiaomi MiMo Provider contract and dual-protocol OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        AsrLanguage, AudioFormat, AudioInputCapabilities, AudioUnderstandingProfile,
        ExecutableAudioProfile, FunctionToolCapabilities, GeneratedAudioCapabilities,
        ImageDetailPolicy, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
        InlineAudioInputLimits, InlineAudioInputProfile, InlineImageInputLimits,
        InlineImageInputProfile, JsonAudioDelivery, JsonAudioFraming, PresetVoiceCapabilities,
        ProviderAudioCeiling, ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, RemoteAudioInputProfile,
        RemoteImageInputLimits, ResponseInclude, SpeechRecognitionProfile, SpeechSynthesisProfile,
        SseAudioDelivery, SseAudioFraming, StructuredOutputProfile, ToolChoiceMode,
        VoiceCloneProfile, VoiceDesignProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
        take_chat_reasoning_switch,
    },
};

pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    64,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: RemoteImageInputLimits::new(8_192),
        data: InlineImageInputProfile::new(
            &[
                ImageMediaType::Jpeg,
                ImageMediaType::Png,
                ImageMediaType::Gif,
                ImageMediaType::Webp,
                ImageMediaType::Bmp,
            ],
            InlineImageInputLimits::new(
                50 * 1024 * 1024,
                38 * 1024 * 1024,
                50 * 1024 * 1024,
                38 * 1024 * 1024,
            ),
        ),
    },
    ImageDetailPolicy::OmittedOnly { default: None },
);

const AUDIO_STREAMING_FORMATS: &[AudioFormat] = &[AudioFormat::Pcm16];
const AUDIO_VOICES: &[&str] = &["mimo_default"];
const ASR_LANGUAGES: &[AsrLanguage] = &[AsrLanguage::Auto, AsrLanguage::Zh, AsrLanguage::En];
const AUDIO_INPUT_FORMATS: &[AudioFormat] = &[
    AudioFormat::Wav,
    AudioFormat::Mp3,
    AudioFormat::Flac,
    AudioFormat::M4a,
    AudioFormat::Ogg,
];
const AUDIO_INPUT_INLINE: InlineAudioInputProfile = InlineAudioInputProfile::new(
    AUDIO_INPUT_FORMATS,
    InlineAudioInputLimits::new(
        10 * 1024 * 1024,
        8 * 1024 * 1024,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
    ),
);
const AUDIO_INPUT: AudioInputCapabilities = AudioInputCapabilities::new(
    64,
    Some(RemoteAudioInputProfile::new(AUDIO_INPUT_FORMATS, 8_192)),
    Some(AUDIO_INPUT_INLINE),
    Some(AUDIO_INPUT_INLINE),
);

const VOICE_CONDITIONING_INLINE: InlineAudioInputProfile = InlineAudioInputProfile::new(
    &[AudioFormat::Wav, AudioFormat::Mp3],
    InlineAudioInputLimits::new(
        10 * 1024 * 1024,
        8 * 1024 * 1024,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
    ),
);
const VOICE_CONDITIONING: AudioInputCapabilities =
    AudioInputCapabilities::new(1, None, Some(VOICE_CONDITIONING_INLINE), None);

const GENERATED_AUDIO_CEILING: GeneratedAudioCapabilities = GeneratedAudioCapabilities::new(
    JsonAudioDelivery::new(
        &[AudioFormat::Wav, AudioFormat::Mp3],
        16 * 1024 * 1024,
        12 * 1024 * 1024,
        JsonAudioFraming::ChatMessageAudioData,
    ),
    SseAudioDelivery::new(
        AUDIO_STREAMING_FORMATS,
        64 * 1024 * 1024,
        SseAudioFraming::ChatDeltaAudioData,
    ),
);

const GENERATED_AUDIO_TARGET: GeneratedAudioCapabilities = GeneratedAudioCapabilities::new(
    JsonAudioDelivery::new(
        &[AudioFormat::Wav],
        16 * 1024 * 1024,
        12 * 1024 * 1024,
        JsonAudioFraming::ChatMessageAudioData,
    ),
    SseAudioDelivery::new(
        AUDIO_STREAMING_FORMATS,
        64 * 1024 * 1024,
        SseAudioFraming::ChatDeltaAudioData,
    ),
);

/// Fixed general audio-understanding profile accepted by the MiMo V2.5 Chat target.
pub(crate) const AUDIO_UNDERSTANDING: ExecutableAudioProfile =
    ExecutableAudioProfile::AudioUnderstanding(AudioUnderstandingProfile::new(
        AudioInputCapabilities::new(
            1,
            None,
            Some(InlineAudioInputProfile::new(
                &[AudioFormat::Wav],
                InlineAudioInputLimits::new(
                    10 * 1024 * 1024,
                    8 * 1024 * 1024,
                    10 * 1024 * 1024,
                    8 * 1024 * 1024,
                ),
            )),
            None,
        ),
    ));

const ASR_INLINE_AUDIO: InlineAudioInputProfile = InlineAudioInputProfile::new(
    &[AudioFormat::Wav],
    InlineAudioInputLimits::new(
        10 * 1024 * 1024,
        8 * 1024 * 1024,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
    ),
);

/// Fixed ASR task profile accepted by the MiMo Chat endpoint.
pub(crate) const ASR_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::SpeechRecognition(SpeechRecognitionProfile::new(
        AudioInputCapabilities::new(1, None, Some(ASR_INLINE_AUDIO), Some(ASR_INLINE_AUDIO)),
        ASR_LANGUAGES,
    ));

/// Fixed ordinary TTS task profile accepted by the MiMo Chat endpoint.
pub(crate) const TTS_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::SpeechSynthesis(SpeechSynthesisProfile::new(
        GENERATED_AUDIO_TARGET,
        PresetVoiceCapabilities::new(AUDIO_VOICES),
    ));

/// Fixed voice-design task profile; a natural-language voice description is carried in Chat text.
pub(crate) const VOICE_DESIGN_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::VoiceDesign(VoiceDesignProfile::new(GENERATED_AUDIO_TARGET));

/// Fixed voice-clone task profile; reference audio is a separate conditioning resource.
pub(crate) const VOICE_CLONE_AUDIO: ExecutableAudioProfile = ExecutableAudioProfile::VoiceClone(
    VoiceCloneProfile::new(VOICE_CONDITIONING, GENERATED_AUDIO_TARGET),
);

/// MiMo Provider-wide audio ceiling with one complete payload per independently supported task.
const AUDIO_CEILING: ProviderAudioCeiling = ProviderAudioCeiling::new(
    ExecutableAudioProfile::AudioUnderstanding(AudioUnderstandingProfile::new(AUDIO_INPUT)),
)
.with(ExecutableAudioProfile::SpeechRecognition(
    SpeechRecognitionProfile::new(AUDIO_INPUT, ASR_LANGUAGES),
))
.with(ExecutableAudioProfile::SpeechSynthesis(
    SpeechSynthesisProfile::new(
        GENERATED_AUDIO_CEILING,
        PresetVoiceCapabilities::new(AUDIO_VOICES),
    ),
))
.with(ExecutableAudioProfile::VoiceDesign(
    VoiceDesignProfile::new(GENERATED_AUDIO_CEILING),
))
.with(ExecutableAudioProfile::VoiceClone(VoiceCloneProfile::new(
    VOICE_CONDITIONING,
    GENERATED_AUDIO_CEILING,
)));

/// Confirmed MiMo Chat and Responses operation surface.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: &[ToolChoiceMode::Auto],
                parallel_calls: true,
                strict_schema: true,
            }),
            media: crate::core::ChatMediaProfile::new(Some(IMAGE_INPUT), Some(AUDIO_CEILING), None),
            structured_outputs: Some(StructuredOutputProfile::JsonObject),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
    )),
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: &[ToolChoiceMode::Auto],
                parallel_calls: true,
                strict_schema: true,
            }),
            media: crate::core::ResponsesMediaProfile::new(Some(IMAGE_INPUT), None),
            structured_outputs: Some(StructuredOutputProfile::JsonObject),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            hosted_tools: &[],
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: true,
            context_management: false,
            include: &[ResponseInclude::ReasoningEncryptedContent],
            moderation: false,
            logprobs: false,
        },
    )),
    None,
);

/// Dual-protocol OpenAI-compatible wire profile used by MiMo.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::MiMo,
    API_SURFACE,
    "/v1/models",
    transform_request_headers,
)
.with_request_body_hook(transform_request_body);

/// Single static descriptor for the MiMo contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves the dedicated hook boundary for future MiMo ordinary-header transforms.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Converts each admitted Chat level to MiMo's documented `thinking.type` switch.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Preserve requests without an explicit Chat level.
    let Some(enabled) = take_chat_reasoning_switch(protocol, document)? else {
        return Ok(());
    };

    // Write the fixed Provider extension after removing the standard downstream field.
    document.insert(
        "thinking".to_owned(),
        serde_json::json!({
            "type": if enabled { "enabled" } else { "disabled" }
        }),
    );
    Ok(())
}
