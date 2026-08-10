//! Static Xiaomi MiMo Provider contract and dual-protocol OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputLimits, AudioInputSource,
        AudioUnderstandingProfile, ExecutableAudioProfile, FunctionToolCapabilities,
        GeneratedAudioCapabilities, ImageDetailPolicy, ImageInputCapabilities, ImageMediaType,
        ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
        JsonAudioDelivery, JsonAudioFraming, PresetVoiceCapabilities, ProviderAudioCeiling,
        ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, RemoteImageInputLimits, ResponseInclude,
        SpeechRecognitionProfile, SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming,
        StructuredOutputProfile, ToolChoiceMode, VoiceCloneProfile, VoiceDesignProfile,
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

const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
    ImageMediaType::Bmp,
];
const IMAGE_REMOTE_LIMITS: RemoteImageInputLimits = RemoteImageInputLimits::new(8_192);
const IMAGE_INLINE_LIMITS: InlineImageInputLimits = InlineImageInputLimits::new(
    50 * 1024 * 1024,
    38 * 1024 * 1024,
    50 * 1024 * 1024,
    38 * 1024 * 1024,
);
const IMAGE_INLINE_PROFILE: InlineImageInputProfile =
    InlineImageInputProfile::new(IMAGE_MEDIA_TYPES, IMAGE_INLINE_LIMITS);
const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    64,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: IMAGE_REMOTE_LIMITS,
        data: IMAGE_INLINE_PROFILE,
    },
    ImageDetailPolicy::OmittedOnly { default: None },
);

const AUDIO_INPUT_SOURCES: &[AudioInputSource] = &[
    AudioInputSource::RemoteUrl,
    AudioInputSource::DataUrl,
    AudioInputSource::Base64,
];
const AUDIO_INPUT_FORMATS: &[AudioFormat] = &[
    AudioFormat::Wav,
    AudioFormat::Mp3,
    AudioFormat::Flac,
    AudioFormat::M4a,
    AudioFormat::Ogg,
];
const AUDIO_OUTPUT_FORMATS: &[AudioFormat] = &[AudioFormat::Wav, AudioFormat::Mp3];
const AUDIO_STREAMING_FORMATS: &[AudioFormat] = &[AudioFormat::Pcm16];
const AUDIO_VOICES: &[&str] = &["mimo_default"];
const ASR_LANGUAGES: &[AsrLanguage] = &[AsrLanguage::Auto, AsrLanguage::Zh, AsrLanguage::En];
const TEXT_TOOL_CHOICE_MODES: &[ToolChoiceMode] = &[ToolChoiceMode::Auto];
const TEXT_FUNCTION_TOOLS: FunctionToolCapabilities = FunctionToolCapabilities {
    choice_modes: TEXT_TOOL_CHOICE_MODES,
    parallel_calls: true,
    strict_schema: true,
};
const TEXT_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;
const TEXT_RESPONSES_INCLUDES: &[ResponseInclude] = &[ResponseInclude::ReasoningEncryptedContent];

const AUDIO_INPUT: AudioInputCapabilities = AudioInputCapabilities::new(
    AUDIO_INPUT_SOURCES,
    AUDIO_INPUT_FORMATS,
    AudioInputLimits::new(
        64,
        8_192,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
    ),
);

const VOICE_CONDITIONING: AudioInputCapabilities = AudioInputCapabilities::new(
    &[AudioInputSource::DataUrl],
    &[AudioFormat::Wav, AudioFormat::Mp3],
    AudioInputLimits::new(
        1,
        0,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
        10 * 1024 * 1024,
        8 * 1024 * 1024,
    ),
);

const GENERATED_AUDIO_CEILING: GeneratedAudioCapabilities = GeneratedAudioCapabilities::new(
    JsonAudioDelivery::new(
        AUDIO_OUTPUT_FORMATS,
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

/// Fixed ASR task profile accepted by the MiMo Chat endpoint.
pub(crate) const ASR_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::SpeechRecognition(SpeechRecognitionProfile::new(
        AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl, AudioInputSource::Base64],
            &[AudioFormat::Wav],
            AudioInputLimits::new(
                1,
                0,
                10 * 1024 * 1024,
                8 * 1024 * 1024,
                10 * 1024 * 1024,
                8 * 1024 * 1024,
            ),
        ),
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
            function_tools: Some(TEXT_FUNCTION_TOOLS),
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(TEXT_STRUCTURED_OUTPUTS),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            audio: Some(AUDIO_CEILING),
            file_input: false,
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
            function_tools: Some(TEXT_FUNCTION_TOOLS),
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(TEXT_STRUCTURED_OUTPUTS),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: false,
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: true,
            context_management: false,
            include: TEXT_RESPONSES_INCLUDES,
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
