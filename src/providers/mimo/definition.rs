//! Static Xiaomi MiMo Provider contract and dual-protocol OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_STRUCTURED_OUTPUT_MODES, ALL_TOOL_CHOICE_MODES, ApiCapabilities, AudioCapabilities,
        AudioFormat, AudioInputCapabilities, AudioInputSource, AudioOutputCapabilities, AudioTask,
        ChatCompletionsCapabilities, FunctionToolCapabilities, ImageInputCapabilities,
        ImageInputSource, ImageMediaType, ReasoningOutput, ResponsesCapabilities,
        StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

const IMAGE_SOURCES: &[ImageInputSource] =
    &[ImageInputSource::RemoteUrl, ImageInputSource::DataUrl];
const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
    ImageMediaType::Bmp,
];
const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities {
    sources: IMAGE_SOURCES,
    media_types: IMAGE_MEDIA_TYPES,
    detail_default: None,
    allowed_details: &[],
    max_parts: 64,
    max_url_length: 8_192,
    max_inline_encoded_bytes: 50 * 1024 * 1024,
    max_inline_decoded_bytes: 38 * 1024 * 1024,
    max_total_inline_encoded_bytes: 50 * 1024 * 1024,
    max_total_inline_decoded_bytes: 38 * 1024 * 1024,
};

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

const AUDIO_INPUT: AudioInputCapabilities = AudioInputCapabilities {
    sources: AUDIO_INPUT_SOURCES,
    formats: AUDIO_INPUT_FORMATS,
    max_parts: 64,
    max_url_length: 8_192,
    max_inline_encoded_bytes: 10 * 1024 * 1024,
    max_inline_decoded_bytes: 8 * 1024 * 1024,
    max_total_inline_encoded_bytes: 10 * 1024 * 1024,
    max_total_inline_decoded_bytes: 8 * 1024 * 1024,
};

const VOICE_CONDITIONING: AudioInputCapabilities = AudioInputCapabilities {
    sources: &[AudioInputSource::DataUrl],
    formats: &[AudioFormat::Wav, AudioFormat::Mp3],
    max_parts: 1,
    max_url_length: 0,
    max_inline_encoded_bytes: 10 * 1024 * 1024,
    max_inline_decoded_bytes: 8 * 1024 * 1024,
    max_total_inline_encoded_bytes: 10 * 1024 * 1024,
    max_total_inline_decoded_bytes: 8 * 1024 * 1024,
};

const AUDIO_OUTPUT: AudioOutputCapabilities = AudioOutputCapabilities {
    formats: AUDIO_OUTPUT_FORMATS,
    streaming_formats: AUDIO_STREAMING_FORMATS,
    voices: AUDIO_VOICES,
    max_inline_encoded_bytes: 16 * 1024 * 1024,
    max_inline_decoded_bytes: 12 * 1024 * 1024,
    max_stream_decoded_bytes: 64 * 1024 * 1024,
};

/// MiMo Provider-wide audio ceiling; each concrete target narrows this to one task profile.
pub(crate) const AUDIO_CEILING: AudioCapabilities = AudioCapabilities {
    task: AudioTask::Any,
    input: Some(AUDIO_INPUT),
    voice_conditioning: Some(VOICE_CONDITIONING),
    output: Some(AUDIO_OUTPUT),
};

/// Fixed ASR task profile accepted by the MiMo Chat endpoint.
pub(crate) const ASR_AUDIO: AudioCapabilities = AudioCapabilities {
    task: AudioTask::Asr,
    input: Some(AudioInputCapabilities {
        sources: &[AudioInputSource::DataUrl, AudioInputSource::Base64],
        formats: &[AudioFormat::Wav],
        max_parts: 1,
        max_url_length: 0,
        max_inline_encoded_bytes: 10 * 1024 * 1024,
        max_inline_decoded_bytes: 8 * 1024 * 1024,
        max_total_inline_encoded_bytes: 10 * 1024 * 1024,
        max_total_inline_decoded_bytes: 8 * 1024 * 1024,
    }),
    voice_conditioning: None,
    output: None,
};

/// Fixed ordinary TTS task profile accepted by the MiMo Chat endpoint.
pub(crate) const TTS_AUDIO: AudioCapabilities = AudioCapabilities {
    task: AudioTask::Tts,
    input: None,
    voice_conditioning: None,
    output: Some(AudioOutputCapabilities {
        formats: &[AudioFormat::Wav],
        streaming_formats: AUDIO_STREAMING_FORMATS,
        voices: AUDIO_VOICES,
        ..AUDIO_OUTPUT
    }),
};

/// Fixed voice-design task profile; a natural-language voice description is carried in Chat text.
pub(crate) const VOICE_DESIGN_AUDIO: AudioCapabilities = AudioCapabilities {
    task: AudioTask::VoiceDesign,
    input: None,
    voice_conditioning: None,
    output: Some(AudioOutputCapabilities {
        formats: &[AudioFormat::Wav],
        voices: &[],
        ..AUDIO_OUTPUT
    }),
};

/// Fixed voice-clone task profile; reference audio is a separate conditioning resource.
pub(crate) const VOICE_CLONE_AUDIO: AudioCapabilities = AudioCapabilities {
    task: AudioTask::VoiceClone,
    input: None,
    voice_conditioning: Some(VOICE_CONDITIONING),
    output: Some(AudioOutputCapabilities {
        formats: &[AudioFormat::Wav],
        voices: &[],
        ..AUDIO_OUTPUT
    }),
};

/// Confirmed MiMo capability ceiling for Chat Completions and Responses; readable reasoning output
/// is not yet confirmed on the wire.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::MiMo,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: true,
            }),
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(StructuredOutputProfile {
                modes: ALL_STRUCTURED_OUTPUT_MODES,
                strict_schema: true,
            }),
            store: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            audio: Some(AUDIO_CEILING),
            file_input: false,
            predicted_outputs: false,
            web_search: false,
            prompt_caching: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: true,
            }),
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(StructuredOutputProfile {
                modes: ALL_STRUCTURED_OUTPUT_MODES,
                strict_schema: true,
            }),
            store: false,
            previous_response_id: false,
            background: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: false,
            conversation: false,
            prompt_templates: false,
            prompt_caching: false,
            context_management: false,
            include: &[],
            moderation: false,
            logprobs: false,
        },
        embeddings: crate::core::EmbeddingsCapabilities::disabled(),
    },
    &[CredentialKind::ApiKey],
);

/// Dual-protocol OpenAI-compatible wire profile used by MiMo.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::MiMo,
    &CONTRACT,
    Some("/v1/chat/completions"),
    Some("/v1/responses"),
    None,
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the MiMo contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Preserves the dedicated hook boundary for future MiMo ordinary-header transforms.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
