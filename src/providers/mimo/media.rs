//! Provider-local image, audio ceiling, and named Target media profiles for MiMo.

use crate::core::{
    AsrLanguage, AudioFormat, AudioInputCapabilities, AudioUnderstandingProfile,
    ExecutableAudioProfile, GeneratedAudioCapabilities, ImageDetailPolicy, ImageInputCapabilities,
    ImageMediaType, ImageSourceCapabilities, InlineAudioInputLimits, InlineAudioInputProfile,
    InlineImageInputLimits, InlineImageInputProfile, JsonAudioDelivery, JsonAudioFraming,
    PresetVoiceCapabilities, ProviderAudioCeiling, RemoteAudioInputProfile, RemoteImageInputLimits,
    SpeechRecognitionProfile, SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming,
    VoiceCloneProfile, VoiceDesignProfile,
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
pub(super) const AUDIO_UNDERSTANDING: ExecutableAudioProfile =
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
pub(super) const ASR_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::SpeechRecognition(SpeechRecognitionProfile::new(
        AudioInputCapabilities::new(1, None, Some(ASR_INLINE_AUDIO), Some(ASR_INLINE_AUDIO)),
        ASR_LANGUAGES,
    ));

/// Fixed ordinary TTS task profile accepted by the MiMo Chat endpoint.
pub(super) const TTS_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::SpeechSynthesis(SpeechSynthesisProfile::new(
        GENERATED_AUDIO_TARGET,
        PresetVoiceCapabilities::new(AUDIO_VOICES),
    ));

/// Fixed voice-design task profile; a natural-language voice description is carried in Chat text.
pub(super) const VOICE_DESIGN_AUDIO: ExecutableAudioProfile =
    ExecutableAudioProfile::VoiceDesign(VoiceDesignProfile::new(GENERATED_AUDIO_TARGET));

/// Fixed voice-clone task profile; reference audio is a separate conditioning resource.
pub(super) const VOICE_CLONE_AUDIO: ExecutableAudioProfile = ExecutableAudioProfile::VoiceClone(
    VoiceCloneProfile::new(VOICE_CONDITIONING, GENERATED_AUDIO_TARGET),
);

/// MiMo Provider-wide audio ceiling with one complete payload per independently supported task.
pub(super) const AUDIO_CEILING: ProviderAudioCeiling = ProviderAudioCeiling::new(
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
