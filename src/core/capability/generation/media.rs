//! Typed image, audio, and file media capability profiles for generation operations.
//!
//! Source-specific limits live with their source payloads. Provider ceilings and executable
//! Target media profiles use the same checked algebra without owning protocol envelope fields.

use serde::Serialize;

/// Standard image source kinds accepted by one protocol-native input profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageInputSource {
    /// An absolute HTTPS URL fetched by the upstream Provider.
    RemoteUrl,
    /// An inline RFC 2397-style Base64 data URL.
    DataUrl,
    /// An opaque Provider-issued file identifier.
    FileId,
}

/// Image media types that OpenBridge can validate without inspecting image content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ImageMediaType {
    /// JPEG image data.
    #[serde(rename = "image/jpeg")]
    Jpeg,
    /// PNG image data.
    #[serde(rename = "image/png")]
    Png,
    /// GIF image data.
    #[serde(rename = "image/gif")]
    Gif,
    /// WebP image data.
    #[serde(rename = "image/webp")]
    Webp,
    /// BMP image data.
    #[serde(rename = "image/bmp")]
    Bmp,
}

impl ImageMediaType {
    /// Parses one canonical image media type without accepting aliases or parameters.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            "image/bmp" => Some(Self::Bmp),
            _ => None,
        }
    }
}

/// Source encodings accepted by a typed audio input profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioInputSource {
    /// An absolute HTTPS URL fetched by the upstream Provider.
    RemoteUrl,
    /// An RFC 2397-style Base64 data URL.
    DataUrl,
    /// A pure Base64 string paired with a separate format field.
    Base64,
}

/// Audio container or wire encoding accepted by an audio profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// RIFF/WAV audio.
    Wav,
    /// MPEG audio.
    Mp3,
    /// FLAC audio.
    Flac,
    /// MPEG-4 audio.
    M4a,
    /// Ogg audio.
    Ogg,
    /// Raw signed 16-bit PCM audio.
    Pcm16,
}

impl AudioFormat {
    /// Parses a canonical model/profile audio format string.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "flac" => Some(Self::Flac),
            "m4a" => Some(Self::M4a),
            "ogg" => Some(Self::Ogg),
            "pcm16" => Some(Self::Pcm16),
            _ => None,
        }
    }
}

/// Rejects duplicate audio-format entries during const profile construction.
const fn assert_unique_audio_formats(formats: &[AudioFormat]) {
    let mut left = 0;
    while left < formats.len() {
        let mut right = left + 1;
        while right < formats.len() {
            assert!(
                formats[left] as usize != formats[right] as usize,
                "audio formats must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
    }
}

/// Rejects duplicate ASR-language entries during const profile construction.
const fn assert_unique_asr_languages(languages: &[AsrLanguage]) {
    let mut left = 0;
    while left < languages.len() {
        let mut right = left + 1;
        while right < languages.len() {
            assert!(
                languages[left] as usize != languages[right] as usize,
                "ASR languages must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
    }
}

/// Compares static protocol strings in const constructors.
const fn static_str_eq(left: &str, right: &str) -> bool {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    if left_bytes.len() != right_bytes.len() {
        return false;
    }

    let mut index = 0;
    while index < left_bytes.len() {
        if left_bytes[index] != right_bytes[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// Remote-URL audio source payload with its own format domain and URL limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteAudioInputProfile {
    formats: &'static [AudioFormat],
    max_url_length: u32,
}

impl RemoteAudioInputProfile {
    /// Creates a checked remote-URL profile.
    pub const fn new(formats: &'static [AudioFormat], max_url_length: u32) -> Self {
        assert!(
            !formats.is_empty(),
            "remote audio formats must not be empty"
        );
        assert_unique_audio_formats(formats);
        assert!(
            max_url_length > 0,
            "remote audio URL limit must be positive"
        );
        Self {
            formats,
            max_url_length,
        }
    }

    /// Returns formats accepted for this remote source.
    pub const fn formats(self) -> &'static [AudioFormat] {
        self.formats
    }

    /// Returns the maximum UTF-8 URL length.
    pub const fn max_url_length(self) -> u32 {
        self.max_url_length
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.formats
            .iter()
            .all(|format| upper.formats.contains(format))
            && self.max_url_length <= upper.max_url_length
    }
}

/// Per-item and cumulative byte budgets owned by one inline audio source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineAudioInputLimits {
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

impl InlineAudioInputLimits {
    /// Creates coherent positive inline audio budgets.
    pub const fn new(
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
    ) -> Self {
        assert!(
            max_inline_encoded_bytes > 0,
            "inline audio encoded-byte limit must be positive"
        );
        assert!(
            max_inline_decoded_bytes > 0,
            "inline audio decoded-byte limit must be positive"
        );
        assert!(
            max_total_inline_encoded_bytes >= max_inline_encoded_bytes,
            "total encoded-byte limit must cover one inline audio resource"
        );
        assert!(
            max_total_inline_decoded_bytes >= max_inline_decoded_bytes,
            "total decoded-byte limit must cover one inline audio resource"
        );
        Self {
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        }
    }

    /// Returns the per-resource encoded-byte limit.
    pub const fn max_inline_encoded_bytes(self) -> u32 {
        self.max_inline_encoded_bytes
    }

    /// Returns the per-resource decoded-byte limit.
    pub const fn max_inline_decoded_bytes(self) -> u32 {
        self.max_inline_decoded_bytes
    }

    /// Returns the cumulative encoded-byte limit.
    pub const fn max_total_inline_encoded_bytes(self) -> u32 {
        self.max_total_inline_encoded_bytes
    }

    /// Returns the cumulative decoded-byte limit.
    pub const fn max_total_inline_decoded_bytes(self) -> u32 {
        self.max_total_inline_decoded_bytes
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.max_inline_encoded_bytes <= upper.max_inline_encoded_bytes
            && self.max_inline_decoded_bytes <= upper.max_inline_decoded_bytes
            && self.max_total_inline_encoded_bytes <= upper.max_total_inline_encoded_bytes
            && self.max_total_inline_decoded_bytes <= upper.max_total_inline_decoded_bytes
    }

    const fn is_reachable_for(self, max_parts: u32) -> bool {
        self.max_total_inline_encoded_bytes as u64
            <= self.max_inline_encoded_bytes as u64 * max_parts as u64
            && self.max_total_inline_decoded_bytes as u64
                <= self.max_inline_decoded_bytes as u64 * max_parts as u64
    }
}

/// Data-URL or pure-Base64 source payload with its own formats and inline budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineAudioInputProfile {
    formats: &'static [AudioFormat],
    limits: InlineAudioInputLimits,
}

impl InlineAudioInputProfile {
    /// Creates one checked inline source profile.
    pub const fn new(formats: &'static [AudioFormat], limits: InlineAudioInputLimits) -> Self {
        assert!(
            !formats.is_empty(),
            "inline audio formats must not be empty"
        );
        assert_unique_audio_formats(formats);
        Self { formats, limits }
    }

    /// Returns formats accepted by this inline source.
    pub const fn formats(self) -> &'static [AudioFormat] {
        self.formats
    }

    /// Returns the complete byte-budget payload for this inline source.
    pub const fn limits(self) -> InlineAudioInputLimits {
        self.limits
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.formats
            .iter()
            .all(|format| upper.formats.contains(format))
            && self.limits.is_subset_of(upper.limits)
    }
}

/// Typed limits and sources for inbound audio resources in one task request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioInputCapabilities {
    max_parts: u32,
    remote_url: Option<RemoteAudioInputProfile>,
    data_url: Option<InlineAudioInputProfile>,
    base64: Option<InlineAudioInputProfile>,
}

impl AudioInputCapabilities {
    /// Creates a complete audio-input profile whose present sources own their formats and limits.
    ///
    /// # Panics
    ///
    /// Panics when `max_parts` is zero, no source is present, or an inline cumulative budget is not
    /// reachable under the request cardinality.
    pub const fn new(
        max_parts: u32,
        remote_url: Option<RemoteAudioInputProfile>,
        data_url: Option<InlineAudioInputProfile>,
        base64: Option<InlineAudioInputProfile>,
    ) -> Self {
        assert!(max_parts > 0, "audio input max_parts must be positive");
        assert!(
            remote_url.is_some() || data_url.is_some() || base64.is_some(),
            "audio input requires at least one source profile"
        );
        if let Some(profile) = data_url {
            assert!(
                profile.limits.is_reachable_for(max_parts),
                "data-URL audio total limits must be reachable"
            );
        }
        if let Some(profile) = base64 {
            assert!(
                profile.limits.is_reachable_for(max_parts),
                "Base64 audio total limits must be reachable"
            );
        }
        Self {
            max_parts,
            remote_url,
            data_url,
            base64,
        }
    }

    /// Returns whether the profile includes one source-owned payload.
    pub const fn supports_source(self, source: AudioInputSource) -> bool {
        match source {
            AudioInputSource::RemoteUrl => self.remote_url.is_some(),
            AudioInputSource::DataUrl => self.data_url.is_some(),
            AudioInputSource::Base64 => self.base64.is_some(),
        }
    }

    /// Returns whether one source accepts the requested format.
    pub fn supports_format(self, source: AudioInputSource, format: AudioFormat) -> bool {
        match source {
            AudioInputSource::RemoteUrl => self
                .remote_url
                .is_some_and(|profile| profile.formats.contains(&format)),
            AudioInputSource::DataUrl => self
                .data_url
                .is_some_and(|profile| profile.formats.contains(&format)),
            AudioInputSource::Base64 => self
                .base64
                .is_some_and(|profile| profile.formats.contains(&format)),
        }
    }

    /// Returns the remote-URL source payload when enabled.
    pub const fn remote_url(self) -> Option<RemoteAudioInputProfile> {
        self.remote_url
    }

    /// Returns the data-URL source payload when enabled.
    pub const fn data_url(self) -> Option<InlineAudioInputProfile> {
        self.data_url
    }

    /// Returns the pure-Base64 source payload when enabled.
    pub const fn base64(self) -> Option<InlineAudioInputProfile> {
        self.base64
    }

    /// Returns the maximum number of audio resources in one task request.
    pub const fn max_parts(self) -> u32 {
        self.max_parts
    }

    /// Returns whether one profile is no broader than this profile.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        self.max_parts <= upper.max_parts
            && optional_remote_audio_is_subset_of(self.remote_url, upper.remote_url)
            && optional_inline_audio_is_subset_of(self.data_url, upper.data_url)
            && optional_inline_audio_is_subset_of(self.base64, upper.base64)
    }
}

fn optional_remote_audio_is_subset_of(
    value: Option<RemoteAudioInputProfile>,
    upper: Option<RemoteAudioInputProfile>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

fn optional_inline_audio_is_subset_of(
    value: Option<InlineAudioInputProfile>,
    upper: Option<InlineAudioInputProfile>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

/// ASR language selections represented by executable speech-recognition profiles.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrLanguage {
    /// Lets the upstream detect the spoken language.
    Auto,
    /// Requests Mandarin Chinese recognition.
    Zh,
    /// Requests English recognition.
    En,
}

/// JSON framing used for generated Chat audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonAudioFraming {
    /// Base64 audio is returned in `message.audio.data`.
    ChatMessageAudioData,
}

/// SSE framing used for generated Chat audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SseAudioFraming {
    /// Ordered Base64 chunks are returned in `delta.audio.data`.
    ChatDeltaAudioData,
}

/// Required non-streaming JSON delivery contract for generated audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonAudioDelivery {
    formats: &'static [AudioFormat],
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    framing: JsonAudioFraming,
}

impl JsonAudioDelivery {
    /// Creates one non-empty JSON delivery contract with positive response budgets.
    ///
    /// # Panics
    ///
    /// Panics when formats are empty or duplicated, or either response budget is zero.
    pub const fn new(
        formats: &'static [AudioFormat],
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        framing: JsonAudioFraming,
    ) -> Self {
        // Validate the format set before accepting its delivery budgets.
        assert!(!formats.is_empty(), "JSON audio formats must not be empty");
        assert_unique_audio_formats(formats);
        assert!(
            max_inline_encoded_bytes > 0,
            "JSON audio encoded budget must be positive"
        );
        assert!(
            max_inline_decoded_bytes > 0,
            "JSON audio decoded budget must be positive"
        );
        Self {
            formats,
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            framing,
        }
    }

    /// Returns formats accepted by non-streaming requests.
    pub const fn formats(self) -> &'static [AudioFormat] {
        self.formats
    }

    /// Returns the maximum Base64 encoded audio size in one JSON response.
    pub const fn max_inline_encoded_bytes(self) -> u32 {
        self.max_inline_encoded_bytes
    }

    /// Returns the maximum decoded audio size in one JSON response.
    pub const fn max_inline_decoded_bytes(self) -> u32 {
        self.max_inline_decoded_bytes
    }

    /// Returns the exact JSON response framing contract.
    pub const fn framing(self) -> JsonAudioFraming {
        self.framing
    }

    /// Returns whether this JSON delivery stays within another delivery ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.formats
            .iter()
            .all(|format| upper.formats.contains(format))
            && self.max_inline_encoded_bytes <= upper.max_inline_encoded_bytes
            && self.max_inline_decoded_bytes <= upper.max_inline_decoded_bytes
            && self.framing == upper.framing
    }
}

/// Required streaming SSE delivery contract for generated audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SseAudioDelivery {
    formats: &'static [AudioFormat],
    max_stream_decoded_bytes: u32,
    framing: SseAudioFraming,
}

impl SseAudioDelivery {
    /// Creates one non-empty SSE delivery contract with a positive cumulative budget.
    ///
    /// # Panics
    ///
    /// Panics when formats are empty or duplicated, or the cumulative budget is zero.
    pub const fn new(
        formats: &'static [AudioFormat],
        max_stream_decoded_bytes: u32,
        framing: SseAudioFraming,
    ) -> Self {
        // Validate the format set before accepting its cumulative delivery budget.
        assert!(!formats.is_empty(), "SSE audio formats must not be empty");
        assert_unique_audio_formats(formats);
        assert!(
            max_stream_decoded_bytes > 0,
            "SSE audio decoded budget must be positive"
        );
        Self {
            formats,
            max_stream_decoded_bytes,
            framing,
        }
    }

    /// Returns formats accepted by streaming requests.
    pub const fn formats(self) -> &'static [AudioFormat] {
        self.formats
    }

    /// Returns the maximum cumulative decoded audio size in one SSE response.
    pub const fn max_stream_decoded_bytes(self) -> u32 {
        self.max_stream_decoded_bytes
    }

    /// Returns the exact SSE response framing contract.
    pub const fn framing(self) -> SseAudioFraming {
        self.framing
    }

    /// Returns whether this SSE delivery stays within another delivery ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.formats
            .iter()
            .all(|format| upper.formats.contains(format))
            && self.max_stream_decoded_bytes <= upper.max_stream_decoded_bytes
            && self.framing == upper.framing
    }
}

/// Complete generated-audio contract with mandatory JSON and SSE delivery modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedAudioCapabilities {
    json: JsonAudioDelivery,
    sse: SseAudioDelivery,
}

impl GeneratedAudioCapabilities {
    /// Creates a generated-audio contract whose two delivery modes are both executable.
    pub const fn new(json: JsonAudioDelivery, sse: SseAudioDelivery) -> Self {
        Self { json, sse }
    }

    /// Returns the non-streaming JSON delivery profile.
    pub const fn json(self) -> JsonAudioDelivery {
        self.json
    }

    /// Returns the streaming SSE delivery profile.
    pub const fn sse(self) -> SseAudioDelivery {
        self.sse
    }

    /// Returns whether both delivery modes stay within another generated-audio ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.json.is_subset_of(upper.json) && self.sse.is_subset_of(upper.sse)
    }
}

/// Non-empty set of preset voices accepted by ordinary speech synthesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresetVoiceCapabilities {
    values: &'static [&'static str],
}

impl PresetVoiceCapabilities {
    /// Creates a non-empty, duplicate-free preset-voice profile with no empty wire value.
    ///
    /// # Panics
    ///
    /// Panics when the set is empty, contains an empty voice, or contains a duplicate.
    pub const fn new(values: &'static [&'static str]) -> Self {
        // Validate every voice wire value before comparing the set for duplicates.
        assert!(!values.is_empty(), "preset voices must not be empty");
        let mut left = 0;
        while left < values.len() {
            assert!(!values[left].is_empty(), "preset voice must not be empty");
            let mut right = left + 1;
            while right < values.len() {
                assert!(
                    !static_str_eq(values[left], values[right]),
                    "preset voices must not contain duplicates"
                );
                right += 1;
            }
            left += 1;
        }
        Self { values }
    }

    /// Returns the supported preset voice names.
    pub const fn values(self) -> &'static [&'static str] {
        self.values
    }

    /// Returns whether every preset voice stays within another voice ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.values.iter().all(|voice| upper.values.contains(voice))
    }
}

/// Executable Chat profile for general audio understanding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioUnderstandingProfile {
    input: AudioInputCapabilities,
}

impl AudioUnderstandingProfile {
    /// Creates an audio-understanding profile with required business-audio input.
    pub const fn new(input: AudioInputCapabilities) -> Self {
        Self { input }
    }

    /// Returns the required business-audio input profile.
    pub const fn input(self) -> AudioInputCapabilities {
        self.input
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.input.is_subset_of(upper.input)
    }
}

/// Executable Chat profile for speech recognition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeechRecognitionProfile {
    input: AudioInputCapabilities,
    languages: &'static [AsrLanguage],
}

impl SpeechRecognitionProfile {
    /// Creates a speech-recognition profile with required input and unique languages.
    ///
    /// # Panics
    ///
    /// Panics when the language set is empty or contains a duplicate.
    pub const fn new(input: AudioInputCapabilities, languages: &'static [AsrLanguage]) -> Self {
        // Validate the language set before constructing the task profile.
        assert!(!languages.is_empty(), "ASR languages must not be empty");
        assert_unique_asr_languages(languages);
        Self { input, languages }
    }

    /// Returns the required speech input profile.
    pub const fn input(self) -> AudioInputCapabilities {
        self.input
    }

    /// Returns the accepted ASR language selections.
    pub const fn languages(self) -> &'static [AsrLanguage] {
        self.languages
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.input.is_subset_of(upper.input)
            && self
                .languages
                .iter()
                .all(|language| upper.languages.contains(language))
    }
}

/// Executable Chat profile for ordinary preset-voice speech synthesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeechSynthesisProfile {
    generated_audio: GeneratedAudioCapabilities,
    preset_voices: PresetVoiceCapabilities,
}

impl SpeechSynthesisProfile {
    /// Creates a synthesis profile with required delivery and preset-voice contracts.
    pub const fn new(
        generated_audio: GeneratedAudioCapabilities,
        preset_voices: PresetVoiceCapabilities,
    ) -> Self {
        Self {
            generated_audio,
            preset_voices,
        }
    }

    /// Returns the required generated-audio delivery profile.
    pub const fn generated_audio(self) -> GeneratedAudioCapabilities {
        self.generated_audio
    }

    /// Returns the non-empty preset-voice profile.
    pub const fn preset_voices(self) -> PresetVoiceCapabilities {
        self.preset_voices
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.generated_audio.is_subset_of(upper.generated_audio)
            && self.preset_voices.is_subset_of(upper.preset_voices)
    }
}

/// Executable Chat profile for voice design from a natural-language description.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoiceDesignProfile {
    generated_audio: GeneratedAudioCapabilities,
}

impl VoiceDesignProfile {
    /// Creates a voice-design profile with required generated-audio delivery.
    pub const fn new(generated_audio: GeneratedAudioCapabilities) -> Self {
        Self { generated_audio }
    }

    /// Returns the required generated-audio delivery profile.
    pub const fn generated_audio(self) -> GeneratedAudioCapabilities {
        self.generated_audio
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.generated_audio.is_subset_of(upper.generated_audio)
    }
}

/// Executable Chat profile for speech synthesis conditioned on reference audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoiceCloneProfile {
    voice_conditioning: AudioInputCapabilities,
    generated_audio: GeneratedAudioCapabilities,
}

impl VoiceCloneProfile {
    /// Creates a voice-clone profile with required conditioning and output delivery.
    pub const fn new(
        voice_conditioning: AudioInputCapabilities,
        generated_audio: GeneratedAudioCapabilities,
    ) -> Self {
        Self {
            voice_conditioning,
            generated_audio,
        }
    }

    /// Returns the required reference-voice conditioning profile.
    pub const fn voice_conditioning(self) -> AudioInputCapabilities {
        self.voice_conditioning
    }

    /// Returns the required generated-audio delivery profile.
    pub const fn generated_audio(self) -> GeneratedAudioCapabilities {
        self.generated_audio
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.voice_conditioning
            .is_subset_of(upper.voice_conditioning)
            && self.generated_audio.is_subset_of(upper.generated_audio)
    }
}

/// Closed executable audio task profile carried by one concrete Target Chat API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableAudioProfile {
    /// General audio input is interpreted as business content and produces text.
    AudioUnderstanding(AudioUnderstandingProfile),
    /// Speech audio is transcribed to text.
    SpeechRecognition(SpeechRecognitionProfile),
    /// Text is synthesized with one of the declared preset voices.
    SpeechSynthesis(SpeechSynthesisProfile),
    /// A natural-language voice description conditions generated speech.
    VoiceDesign(VoiceDesignProfile),
    /// Reference audio conditions generated speech.
    VoiceClone(VoiceCloneProfile),
}

impl ExecutableAudioProfile {
    /// Returns whether this task consumes business audio or reference-voice input.
    pub const fn has_input(self) -> bool {
        matches!(
            self,
            Self::AudioUnderstanding(_) | Self::SpeechRecognition(_) | Self::VoiceClone(_)
        )
    }

    /// Returns whether this task produces generated audio.
    pub const fn has_output(self) -> bool {
        matches!(
            self,
            Self::SpeechSynthesis(_) | Self::VoiceDesign(_) | Self::VoiceClone(_)
        )
    }

    /// Returns whether the concrete task and payload stay within a Provider audio ceiling.
    fn is_subset_of(self, upper: ProviderAudioCeiling) -> bool {
        upper.contains(self)
    }
}

/// Non-empty Provider audio ceiling with one independent payload slot per task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderAudioCeiling {
    audio_understanding: Option<AudioUnderstandingProfile>,
    speech_recognition: Option<SpeechRecognitionProfile>,
    speech_synthesis: Option<SpeechSynthesisProfile>,
    voice_design: Option<VoiceDesignProfile>,
    voice_clone: Option<VoiceCloneProfile>,
}

impl ProviderAudioCeiling {
    /// Creates a non-empty ceiling from its first complete task profile.
    pub const fn new(first: ExecutableAudioProfile) -> Self {
        Self {
            audio_understanding: None,
            speech_recognition: None,
            speech_synthesis: None,
            voice_design: None,
            voice_clone: None,
        }
        .with(first)
    }

    /// Adds one complete task profile and rejects a duplicate task at construction time.
    ///
    /// # Panics
    ///
    /// Panics when the ceiling already contains the supplied task variant.
    pub const fn with(mut self, profile: ExecutableAudioProfile) -> Self {
        match profile {
            ExecutableAudioProfile::AudioUnderstanding(profile) => {
                assert!(
                    self.audio_understanding.is_none(),
                    "duplicate audio-understanding Provider ceiling"
                );
                self.audio_understanding = Some(profile);
            }
            ExecutableAudioProfile::SpeechRecognition(profile) => {
                assert!(
                    self.speech_recognition.is_none(),
                    "duplicate speech-recognition Provider ceiling"
                );
                self.speech_recognition = Some(profile);
            }
            ExecutableAudioProfile::SpeechSynthesis(profile) => {
                assert!(
                    self.speech_synthesis.is_none(),
                    "duplicate speech-synthesis Provider ceiling"
                );
                self.speech_synthesis = Some(profile);
            }
            ExecutableAudioProfile::VoiceDesign(profile) => {
                assert!(
                    self.voice_design.is_none(),
                    "duplicate voice-design Provider ceiling"
                );
                self.voice_design = Some(profile);
            }
            ExecutableAudioProfile::VoiceClone(profile) => {
                assert!(
                    self.voice_clone.is_none(),
                    "duplicate voice-clone Provider ceiling"
                );
                self.voice_clone = Some(profile);
            }
        }
        self
    }

    /// Returns whether the same task variant and its payload stay inside this ceiling.
    pub(crate) fn contains(self, profile: ExecutableAudioProfile) -> bool {
        match profile {
            ExecutableAudioProfile::AudioUnderstanding(profile) => self
                .audio_understanding
                .is_some_and(|upper| profile.is_subset_of(upper)),
            ExecutableAudioProfile::SpeechRecognition(profile) => self
                .speech_recognition
                .is_some_and(|upper| profile.is_subset_of(upper)),
            ExecutableAudioProfile::SpeechSynthesis(profile) => self
                .speech_synthesis
                .is_some_and(|upper| profile.is_subset_of(upper)),
            ExecutableAudioProfile::VoiceDesign(profile) => self
                .voice_design
                .is_some_and(|upper| profile.is_subset_of(upper)),
            ExecutableAudioProfile::VoiceClone(profile) => self
                .voice_clone
                .is_some_and(|upper| profile.is_subset_of(upper)),
        }
    }
}

/// Standard image-detail values carried by Chat or Responses image parts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    /// Lets the upstream choose the effective image detail.
    Auto,
    /// Requests a low-detail representation.
    Low,
    /// Requests a high-detail representation.
    High,
    /// Requests the original image resolution when supported.
    Original,
}

impl ImageDetail {
    /// Parses one standard image-detail wire value.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "low" => Some(Self::Low),
            "high" => Some(Self::High),
            "original" => Some(Self::Original),
            _ => None,
        }
    }
}

/// Rejects duplicate image-media entries during const profile construction.
const fn assert_unique_image_media_types(media_types: &[ImageMediaType]) {
    let mut left = 0;
    while left < media_types.len() {
        let mut right = left + 1;
        while right < media_types.len() {
            assert!(
                media_types[left] as usize != media_types[right] as usize,
                "image media types must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
    }
}

/// Rejects duplicate explicit image-detail entries during const profile construction.
const fn assert_unique_image_details(details: &[ImageDetail]) {
    let mut left = 0;
    while left < details.len() {
        let mut right = left + 1;
        while right < details.len() {
            assert!(
                details[left] as usize != details[right] as usize,
                "image details must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
    }
}

/// URL-length payload large enough for one absolute HTTPS image reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteImageInputLimits {
    max_url_length: u32,
}

impl RemoteImageInputLimits {
    /// Creates a remote-image limit that can hold at least the shortest absolute HTTPS URL.
    ///
    /// # Panics
    ///
    /// Panics when `max_url_length` is shorter than the nine-byte URL `https://a`.
    pub const fn new(max_url_length: u32) -> Self {
        assert!(
            max_url_length >= 9,
            "remote image URL length limit must allow the nine-byte URL https://a"
        );
        Self { max_url_length }
    }

    /// Returns the maximum UTF-8 byte length of one remote image URL.
    pub const fn max_url_length(self) -> u32 {
        self.max_url_length
    }

    /// Returns whether this remote payload stays within another payload ceiling.
    const fn is_subset_of(self, upper: Self) -> bool {
        self.max_url_length <= upper.max_url_length
    }
}

/// Wire-reachable per-item and cumulative budgets for inline image data URLs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineImageInputLimits {
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

impl InlineImageInputLimits {
    /// Creates coherent inline-image budgets independent of request cardinality.
    ///
    /// [`ImageInputCapabilities::new`] additionally verifies that each cumulative budget is
    /// reachable under the enclosing positive `max_parts` limit.
    ///
    /// # Panics
    ///
    /// Panics when the encoded budget cannot hold one four-byte Base64 quantum, the decoded budget
    /// cannot hold one byte, or a cumulative limit cannot cover one image.
    pub const fn new(
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
    ) -> Self {
        // Validate wire-reachable per-item budgets before relating cumulative capacity.
        assert!(
            max_inline_encoded_bytes >= 4,
            "inline image encoded-byte limit must allow one four-byte Base64 quantum"
        );
        assert!(
            max_inline_decoded_bytes >= 1,
            "inline image decoded-byte limit must allow one byte"
        );
        assert!(
            max_total_inline_encoded_bytes >= max_inline_encoded_bytes,
            "total encoded-byte limit must cover one inline image"
        );
        assert!(
            max_total_inline_decoded_bytes >= max_inline_decoded_bytes,
            "total decoded-byte limit must cover one inline image"
        );

        // Construct the source-local limits after their intrinsic invariants hold.
        Self {
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        }
    }

    /// Returns the maximum Base64 payload length of one inline image.
    pub const fn max_inline_encoded_bytes(self) -> u32 {
        self.max_inline_encoded_bytes
    }

    /// Returns the maximum decoded length of one inline image.
    pub const fn max_inline_decoded_bytes(self) -> u32 {
        self.max_inline_decoded_bytes
    }

    /// Returns the cumulative Base64 payload limit across inline images.
    pub const fn max_total_inline_encoded_bytes(self) -> u32 {
        self.max_total_inline_encoded_bytes
    }

    /// Returns the cumulative decoded-byte limit across inline images.
    pub const fn max_total_inline_decoded_bytes(self) -> u32 {
        self.max_total_inline_decoded_bytes
    }

    /// Verifies that cumulative limits are reachable under the enclosing part count.
    const fn assert_reachable(self, max_parts: u32) {
        assert!(
            self.max_total_inline_encoded_bytes as u64
                <= self.max_inline_encoded_bytes as u64 * max_parts as u64,
            "total encoded-byte limit exceeds the image per-part capacity"
        );
        assert!(
            self.max_total_inline_decoded_bytes as u64
                <= self.max_inline_decoded_bytes as u64 * max_parts as u64,
            "total decoded-byte limit exceeds the image per-part capacity"
        );
    }

    /// Returns whether this inline budget stays within another budget ceiling.
    const fn is_subset_of(self, upper: Self) -> bool {
        self.max_inline_encoded_bytes <= upper.max_inline_encoded_bytes
            && self.max_inline_decoded_bytes <= upper.max_inline_decoded_bytes
            && self.max_total_inline_encoded_bytes <= upper.max_total_inline_encoded_bytes
            && self.max_total_inline_decoded_bytes <= upper.max_total_inline_decoded_bytes
    }
}

/// Non-empty media-type set and complete budgets for inline image data URLs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineImageInputProfile {
    media_types: &'static [ImageMediaType],
    limits: InlineImageInputLimits,
}

impl InlineImageInputProfile {
    /// Creates a checked inline-image profile.
    ///
    /// # Panics
    ///
    /// Panics when `media_types` is empty or contains duplicates.
    pub const fn new(
        media_types: &'static [ImageMediaType],
        limits: InlineImageInputLimits,
    ) -> Self {
        // Validate the set-valued media domain before binding its checked limits.
        assert!(
            !media_types.is_empty(),
            "inline image media types must not be empty"
        );
        assert_unique_image_media_types(media_types);

        // Construct the complete inline source payload.
        Self {
            media_types,
            limits,
        }
    }

    /// Returns the accepted inline image media types.
    pub const fn media_types(self) -> &'static [ImageMediaType] {
        self.media_types
    }

    /// Returns the complete inline-image budgets.
    pub const fn limits(self) -> InlineImageInputLimits {
        self.limits
    }

    /// Returns whether this inline payload stays within another payload ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.media_types
            .iter()
            .all(|media_type| upper.media_types.contains(media_type))
            && self.limits.is_subset_of(upper.limits)
    }
}

/// Closed source-payload union shared by Provider ceilings and executable image profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageSourceCapabilities {
    /// Accepts remote HTTPS URLs with the supplied URL budget.
    RemoteUrl(RemoteImageInputLimits),
    /// Accepts inline data URLs with the supplied media and byte budgets.
    DataUrl(InlineImageInputProfile),
    /// Accepts both implemented source kinds with independently owned payloads.
    RemoteUrlAndDataUrl {
        /// Complete remote-URL payload.
        remote: RemoteImageInputLimits,
        /// Complete inline data-URL payload.
        data: InlineImageInputProfile,
    },
}

impl ImageSourceCapabilities {
    /// Returns the remote-URL payload when this source union accepts remote images.
    pub const fn remote(self) -> Option<RemoteImageInputLimits> {
        match self {
            Self::RemoteUrl(remote) | Self::RemoteUrlAndDataUrl { remote, .. } => Some(remote),
            Self::DataUrl(_) => None,
        }
    }

    /// Returns the inline data-URL payload when this source union accepts inline images.
    pub const fn data(self) -> Option<InlineImageInputProfile> {
        match self {
            Self::DataUrl(data) | Self::RemoteUrlAndDataUrl { data, .. } => Some(data),
            Self::RemoteUrl(_) => None,
        }
    }

    /// Verifies every inline payload against the enclosing positive part count.
    const fn assert_reachable(self, max_parts: u32) {
        if let Some(data) = self.data() {
            data.limits.assert_reachable(max_parts);
        }
    }

    /// Returns whether each retained source payload stays within the same source in the ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        // Compare the remote payload only when the narrower profile retains that source.
        let remote_is_subset = match self.remote() {
            Some(remote) => upper
                .remote()
                .is_some_and(|upper| remote.is_subset_of(upper)),
            None => true,
        };

        // Compare the inline payload independently so one source cannot satisfy another.
        let data_is_subset = match self.data() {
            Some(data) => upper.data().is_some_and(|upper| data.is_subset_of(upper)),
            None => true,
        };
        remote_is_subset && data_is_subset
    }
}

/// Explicit image-detail domain plus the known behavior when `detail` is omitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageDetailProfile {
    default: Option<ImageDetail>,
    allowed: &'static [ImageDetail],
}

impl ImageDetailProfile {
    /// Creates a checked explicit image-detail profile.
    ///
    /// The omitted default is independent of the explicit domain and need not be a member.
    ///
    /// # Panics
    ///
    /// Panics when `allowed` is empty or contains duplicates.
    pub const fn new(default: Option<ImageDetail>, allowed: &'static [ImageDetail]) -> Self {
        assert!(
            !allowed.is_empty(),
            "explicit image details must not be empty"
        );
        assert_unique_image_details(allowed);
        Self { default, allowed }
    }

    /// Returns the known effective detail when the wire field is omitted.
    pub const fn default(self) -> Option<ImageDetail> {
        self.default
    }

    /// Returns the non-empty explicit image-detail domain.
    pub const fn allowed(self) -> &'static [ImageDetail] {
        self.allowed
    }

    /// Returns whether this explicit profile stays within another profile ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.default == upper.default
            && self
                .allowed
                .iter()
                .all(|detail| upper.allowed.contains(detail))
    }
}

/// Closed omitted-only or explicit image-detail request policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageDetailPolicy {
    /// Only omission is accepted, with an optional known effective default.
    OmittedOnly {
        /// Known effective detail when the request omits the wire field.
        default: Option<ImageDetail>,
    },
    /// Explicit values are accepted according to the checked profile.
    Explicit(ImageDetailProfile),
}

impl ImageDetailPolicy {
    /// Returns the known effective detail when the wire field is omitted.
    pub const fn default(self) -> Option<ImageDetail> {
        match self {
            Self::OmittedOnly { default } => default,
            Self::Explicit(profile) => profile.default,
        }
    }

    /// Returns the explicit profile, or `None` for an omitted-only policy.
    pub const fn explicit(self) -> Option<ImageDetailProfile> {
        match self {
            Self::OmittedOnly { .. } => None,
            Self::Explicit(profile) => Some(profile),
        }
    }

    /// Returns whether this detail policy stays within another policy ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        // Preserve the exact known behavior of an omitted wire field.
        if self.default() != upper.default() {
            return false;
        }

        // Apply the closed omitted-only versus explicit subset matrix.
        match (self, upper) {
            (Self::OmittedOnly { .. }, _) => true,
            (Self::Explicit(value), Self::Explicit(upper)) => value.is_subset_of(upper),
            (Self::Explicit(_), Self::OmittedOnly { .. }) => false,
        }
    }
}

/// Provider or Upstream API ceiling for protocol-native image inputs.
///
/// Every accepted source owns its complete payload. Byte limits apply to the Base64 payload after
/// the data-URL prefix. The gateway request-body limit remains an independent deployment-wide
/// ceiling and may be smaller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageInputCapabilities {
    max_parts: u32,
    sources: ImageSourceCapabilities,
    detail_policy: ImageDetailPolicy,
}

impl ImageInputCapabilities {
    /// Creates a complete image-input envelope and validates cross-field reachability.
    ///
    /// # Panics
    ///
    /// Panics when `max_parts` is zero or an inline cumulative budget is unreachable.
    pub const fn new(
        max_parts: u32,
        sources: ImageSourceCapabilities,
        detail_policy: ImageDetailPolicy,
    ) -> Self {
        // Validate outer cardinality before checking source-specific aggregate reachability.
        assert!(max_parts > 0, "image input max_parts must be positive");
        sources.assert_reachable(max_parts);

        // Construct the closed envelope after every primitive and cross-field invariant holds.
        Self {
            max_parts,
            sources,
            detail_policy,
        }
    }

    /// Returns the maximum number of image parts in one request.
    pub const fn max_parts(self) -> u32 {
        self.max_parts
    }

    /// Returns the closed source-payload union.
    pub const fn sources(self) -> ImageSourceCapabilities {
        self.sources
    }

    /// Returns the omitted-only or explicit image-detail policy.
    pub const fn detail_policy(self) -> ImageDetailPolicy {
        self.detail_policy
    }

    /// Returns whether this profile stays within another Provider or API ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.max_parts <= upper.max_parts
            && self.sources.is_subset_of(upper.sources)
            && self.detail_policy.is_subset_of(upper.detail_policy)
    }
}

/// Returns whether one optional image profile is conservatively bounded by another.
pub(super) fn image_input_is_subset_of(
    value: Option<ImageInputCapabilities>,
    upper: Option<ImageInputCapabilities>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
        (Some(_), None) => false,
    }
}

/// Reserved typed contract for Chat Completions `file` content parts.
///
/// The type intentionally has no public constructor until the downstream file wire, limits, and
/// preflight contract are implemented. `Option::None` is therefore the only executable profile in
/// the current release while preserving an operation-specific expansion point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatFileInputProfile {
    _reserved: (),
}

/// Reserved typed contract for Responses `input_file` items and content parts.
///
/// This remains distinct from [`ChatFileInputProfile`] because the two APIs have different source
/// unions. It has no public constructor until those wire contracts are implemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResponsesFileInputProfile {
    _reserved: (),
}

/// Complete Chat Completions media contract selected by a Provider ceiling or executable Target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChatMediaProfile<A> {
    /// Typed `image_url` input profile, or `None` when images are unsupported.
    pub image: Option<ImageInputCapabilities>,
    /// Layer-specific Provider audio ceiling or executable audio profile.
    pub audio: A,
    /// Typed `file` input profile, or `None` while file input is unsupported.
    pub file: Option<ChatFileInputProfile>,
}

impl<A> ChatMediaProfile<A> {
    /// Creates one complete Chat media contract without inheriting another layer's fields.
    pub const fn new(
        image: Option<ImageInputCapabilities>,
        audio: A,
        file: Option<ChatFileInputProfile>,
    ) -> Self {
        Self { image, audio, file }
    }
}

fn optional_executable_audio_is_subset_of(
    value: Option<ExecutableAudioProfile>,
    upper: Option<ProviderAudioCeiling>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

impl ChatMediaProfile<Option<ExecutableAudioProfile>> {
    /// Returns whether one executable Target media contract stays within the Provider ceiling.
    pub(super) fn is_subset_of(
        self,
        upper: ChatMediaProfile<Option<ProviderAudioCeiling>>,
    ) -> bool {
        image_input_is_subset_of(self.image, upper.image)
            && optional_executable_audio_is_subset_of(self.audio, upper.audio)
            && optional_chat_file_input_is_subset_of(self.file, upper.file)
    }
}

/// Complete Responses media contract selected by a Provider ceiling or executable Target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResponsesMediaProfile {
    /// Typed `input_image` profile, or `None` when images are unsupported.
    pub image: Option<ImageInputCapabilities>,
    /// Typed `input_file` profile, or `None` while file input is unsupported.
    pub file: Option<ResponsesFileInputProfile>,
}

impl ResponsesMediaProfile {
    /// Creates one complete Responses media contract without inheriting Provider fields.
    pub const fn new(
        image: Option<ImageInputCapabilities>,
        file: Option<ResponsesFileInputProfile>,
    ) -> Self {
        Self { image, file }
    }

    /// Returns whether one executable Target media contract stays within the Provider ceiling.
    pub(super) fn is_subset_of(self, upper: Self) -> bool {
        image_input_is_subset_of(self.image, upper.image)
            && optional_responses_file_input_is_subset_of(self.file, upper.file)
    }
}

fn optional_chat_file_input_is_subset_of(
    value: Option<ChatFileInputProfile>,
    upper: Option<ChatFileInputProfile>,
) -> bool {
    value.is_none() || upper.is_some()
}

fn optional_responses_file_input_is_subset_of(
    value: Option<ResponsesFileInputProfile>,
    upper: Option<ResponsesFileInputProfile>,
) -> bool {
    value.is_none() || upper.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_input_sources_own_formats_and_limits_without_zero_sentinels() {
        let remote = RemoteAudioInputProfile::new(&[AudioFormat::Mp3], 4_096);
        let data = InlineAudioInputProfile::new(
            &[AudioFormat::Wav],
            InlineAudioInputLimits::new(1_024, 768, 2_048, 1_536),
        );
        let profile = AudioInputCapabilities::new(2, Some(remote), Some(data), None);

        assert!(profile.supports_format(AudioInputSource::RemoteUrl, AudioFormat::Mp3));
        assert!(!profile.supports_format(AudioInputSource::RemoteUrl, AudioFormat::Wav));
        assert!(profile.supports_format(AudioInputSource::DataUrl, AudioFormat::Wav));
        assert_eq!(profile.remote_url().unwrap().max_url_length(), 4_096);
        assert_eq!(
            profile
                .data_url()
                .unwrap()
                .limits()
                .max_inline_decoded_bytes(),
            768
        );
        let narrow_data = InlineAudioInputProfile::new(
            &[AudioFormat::Wav],
            InlineAudioInputLimits::new(1_024, 768, 1_024, 768),
        );
        assert!(
            AudioInputCapabilities::new(1, None, Some(narrow_data), None).is_subset_of(profile)
        );
    }
}
