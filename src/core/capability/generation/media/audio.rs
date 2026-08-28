//! Typed audio capability profiles for generation operations.

use serde::Serialize;

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
    pub(super) fn is_subset_of(self, upper: ProviderAudioCeiling) -> bool {
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
