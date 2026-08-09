//! Chat Completions and Responses capability ceilings.
//!
//! This module owns generation-only fields and the common projection used for subset checks.
//! Reserved protocol positions fail closed until their runtime semantics exist.

use serde::Serialize;

/// Observable output type for upstream-generated reasoning.
///
/// `Unknown` means that wire evidence is insufficient to treat the output as readable text;
/// `Opaque` covers unreadable Provider-issued continuations such as Responses
/// `encrypted_content`. Only `PlainText` and `Summary` can enter a cross-protocol reasoning
/// channel, and the convertible direction remains protocol-specific.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningOutput {
    /// Upstream wire evidence is insufficient to determine the output format.
    #[default]
    Unknown,
    /// The upstream explicitly returns no reasoning output.
    Unsupported,
    /// The upstream returns readable complete reasoning text.
    PlainText,
    /// The upstream returns only a readable reasoning summary.
    Summary,
    /// The upstream returns an unreadable opaque or encrypted continuation.
    Opaque,
}

impl ReasoningOutput {
    /// Returns whether this output contains readable reasoning text or a summary.
    pub const fn is_readable(self) -> bool {
        matches!(self, Self::PlainText | Self::Summary)
    }

    /// Returns whether this configuration claims no additional reasoning output capability over the Provider contract.
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        matches!(
            (self, upper),
            (Self::Unknown | Self::Unsupported, _)
                | (Self::PlainText, Self::PlainText)
                | (Self::Summary, Self::Summary)
                | (Self::Opaque, Self::Opaque)
        )
    }
}

/// OpenAI-hosted tool kinds that a Responses Create request can reference.
///
/// These variants reserve standard protocol positions; the current pipeline, adapters, and Provider
/// registrations do not implement these tools.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedToolKind {
    /// Web search tool.
    WebSearch,
    /// File search tool.
    FileSearch,
    /// Code Interpreter tool.
    CodeInterpreter,
    /// Computer Use tool.
    ComputerUse,
    /// Image generation tool.
    ImageGeneration,
    /// Remote MCP tool.
    Mcp,
    /// Hosted shell tool.
    Shell,
    /// Apply patch tool.
    ApplyPatch,
    /// Tool search tool.
    ToolSearch,
    /// Skills tool.
    Skills,
    /// Programmatic Tool Calling tool.
    ProgrammaticToolCalling,
}

/// Standard additional output kinds for the Responses Create `include` field.
///
/// Variants use descriptive Rust names and Rustdoc identifies their wire paths; they currently
/// serve as reserved interface positions.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseInclude {
    /// `web_search_call.action.sources`.
    WebSearchCallSources,
    /// `code_interpreter_call.outputs`.
    CodeInterpreterCallOutputs,
    /// `computer_call_output.output.image_url`.
    ComputerCallOutputImageUrl,
    /// `file_search_call.results`.
    FileSearchCallResults,
    /// `message.input_image.image_url`.
    InputImageImageUrl,
    /// `message.output_text.logprobs`.
    OutputTextLogprobs,
    /// `reasoning.encrypted_content`.
    ReasoningEncryptedContent,
}

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

/// Returns whether a source set contains one source without relying on non-const slice helpers.
const fn contains_audio_input_source(
    sources: &[AudioInputSource],
    expected: AudioInputSource,
) -> bool {
    let mut index = 0;
    while index < sources.len() {
        if sources[index] as usize == expected as usize {
            return true;
        }
        index += 1;
    }
    false
}

/// Rejects empty or duplicate source sets during const profile construction.
const fn assert_unique_audio_input_sources(sources: &[AudioInputSource]) {
    assert!(!sources.is_empty(), "audio input sources must not be empty");
    let mut left = 0;
    while left < sources.len() {
        let mut right = left + 1;
        while right < sources.len() {
            assert!(
                sources[left] as usize != sources[right] as usize,
                "audio input sources must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
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

/// Coherent cardinality and byte limits used to construct one audio-input profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioInputLimits {
    max_parts: u32,
    max_url_length: u32,
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

impl AudioInputLimits {
    /// Creates source-agnostic limits whose nonzero groups are internally coherent.
    ///
    /// `max_url_length` is zero when remote URLs are absent. All four inline limits are zero when
    /// data URLs and pure Base64 are absent. [`AudioInputCapabilities::new`] binds those groups to
    /// the exact declared source set.
    ///
    /// # Panics
    ///
    /// Panics when `max_parts` is zero, an inline group is partial, or an aggregate inline budget
    /// is outside the reachable per-part capacity.
    pub const fn new(
        max_parts: u32,
        max_url_length: u32,
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
    ) -> Self {
        // Validate request cardinality independently of the source set that will consume it.
        assert!(max_parts > 0, "audio input max_parts must be positive");
        let has_any_inline_limit = max_inline_encoded_bytes > 0
            || max_inline_decoded_bytes > 0
            || max_total_inline_encoded_bytes > 0
            || max_total_inline_decoded_bytes > 0;

        // Validate inline limits as an all-or-nothing bounded group.
        if has_any_inline_limit {
            assert!(
                max_inline_encoded_bytes > 0,
                "inline audio input requires a positive encoded-byte limit"
            );
            assert!(
                max_inline_decoded_bytes > 0,
                "inline audio input requires a positive decoded-byte limit"
            );
            assert!(
                max_total_inline_encoded_bytes >= max_inline_encoded_bytes,
                "total encoded-byte limit must cover one inline audio resource"
            );
            assert!(
                max_total_inline_decoded_bytes >= max_inline_decoded_bytes,
                "total decoded-byte limit must cover one inline audio resource"
            );
            assert!(
                max_total_inline_encoded_bytes as u64
                    <= max_inline_encoded_bytes as u64 * max_parts as u64,
                "total encoded-byte limit exceeds the declared per-part capacity"
            );
            assert!(
                max_total_inline_decoded_bytes as u64
                    <= max_inline_decoded_bytes as u64 * max_parts as u64,
                "total decoded-byte limit exceeds the declared per-part capacity"
            );
        }

        // Construct the reusable limit group after all numeric invariants hold.
        Self {
            max_parts,
            max_url_length,
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        }
    }

    /// Returns whether the complete inline limit group is present.
    const fn has_inline_limits(self) -> bool {
        self.max_inline_encoded_bytes > 0
    }
}

/// Typed limits and sources for inbound audio resources in one task request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioInputCapabilities {
    sources: &'static [AudioInputSource],
    formats: &'static [AudioFormat],
    limits: AudioInputLimits,
}

impl AudioInputCapabilities {
    /// Creates a complete audio-input profile with coherent source-specific limits.
    ///
    /// # Panics
    ///
    /// Panics when a set is empty or contains duplicates, or when the declared sources do not
    /// exactly match the remote and inline groups represented by `limits`.
    pub const fn new(
        sources: &'static [AudioInputSource],
        formats: &'static [AudioFormat],
        limits: AudioInputLimits,
    ) -> Self {
        // Validate the set-valued input dimensions.
        assert_unique_audio_input_sources(sources);
        assert!(!formats.is_empty(), "audio input formats must not be empty");
        assert_unique_audio_formats(formats);

        // Bind remote-URL source presence to the checked URL limit group.
        let accepts_remote = contains_audio_input_source(sources, AudioInputSource::RemoteUrl);
        if accepts_remote {
            assert!(
                limits.max_url_length > 0,
                "remote audio input requires a positive URL length limit"
            );
        } else {
            assert!(
                limits.max_url_length == 0,
                "audio input without remote URLs must have a zero URL length limit"
            );
        }

        // Bind inline source presence to the checked inline limit group.
        let accepts_inline = contains_audio_input_source(sources, AudioInputSource::DataUrl)
            || contains_audio_input_source(sources, AudioInputSource::Base64);
        if accepts_inline {
            assert!(
                limits.has_inline_limits(),
                "inline audio input requires inline byte limits"
            );
        } else {
            assert!(
                !limits.has_inline_limits(),
                "audio input without inline sources must have zero inline limits"
            );
        }

        // Construct a profile whose private fields now satisfy all primitive invariants.
        Self {
            sources,
            formats,
            limits,
        }
    }

    /// Returns the accepted URL, data-URL, or pure-Base64 source forms.
    pub const fn sources(self) -> &'static [AudioInputSource] {
        self.sources
    }

    /// Returns the accepted audio formats.
    pub const fn formats(self) -> &'static [AudioFormat] {
        self.formats
    }

    /// Returns the maximum number of audio resources in one task request.
    pub const fn max_parts(self) -> u32 {
        self.limits.max_parts
    }

    /// Returns the maximum UTF-8 byte length of one remote URL.
    pub const fn max_url_length(self) -> u32 {
        self.limits.max_url_length
    }

    /// Returns the maximum encoded size of one inline resource.
    pub const fn max_inline_encoded_bytes(self) -> u32 {
        self.limits.max_inline_encoded_bytes
    }

    /// Returns the maximum decoded size of one inline resource.
    pub const fn max_inline_decoded_bytes(self) -> u32 {
        self.limits.max_inline_decoded_bytes
    }

    /// Returns the maximum encoded size of all inline resources.
    pub const fn max_total_inline_encoded_bytes(self) -> u32 {
        self.limits.max_total_inline_encoded_bytes
    }

    /// Returns the maximum decoded size of all inline resources.
    pub const fn max_total_inline_decoded_bytes(self) -> u32 {
        self.limits.max_total_inline_decoded_bytes
    }

    /// Returns whether one profile is no broader than this profile.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        self.sources
            .iter()
            .all(|source| upper.sources.contains(source))
            && self
                .formats
                .iter()
                .all(|format| upper.formats.contains(format))
            && self.limits.max_parts <= upper.limits.max_parts
            && self.limits.max_url_length <= upper.limits.max_url_length
            && self.limits.max_inline_encoded_bytes <= upper.limits.max_inline_encoded_bytes
            && self.limits.max_inline_decoded_bytes <= upper.limits.max_inline_decoded_bytes
            && self.limits.max_total_inline_encoded_bytes
                <= upper.limits.max_total_inline_encoded_bytes
            && self.limits.max_total_inline_decoded_bytes
                <= upper.limits.max_total_inline_decoded_bytes
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

/// Function-tool selection modes accepted by a generation operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoiceMode {
    /// Prevents the model from calling tools.
    None,
    /// Lets the model decide whether to call a tool.
    Auto,
    /// Requires the model to call at least one tool.
    Required,
    /// Selects a named function.
    Named,
}

/// All function-tool choice modes currently represented by the gateway contract.
pub const ALL_TOOL_CHOICE_MODES: &[ToolChoiceMode] = &[
    ToolChoiceMode::None,
    ToolChoiceMode::Auto,
    ToolChoiceMode::Required,
    ToolChoiceMode::Named,
];

/// Structured-output modes accepted by a generation operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputMode {
    /// JSON object output constraint.
    JsonObject,
    /// JSON Schema output constraint.
    JsonSchema,
}

const JSON_OBJECT_MODE: &[StructuredOutputMode] = &[StructuredOutputMode::JsonObject];
const JSON_SCHEMA_MODE: &[StructuredOutputMode] = &[StructuredOutputMode::JsonSchema];
const JSON_OBJECT_AND_SCHEMA_MODES: &[StructuredOutputMode] = &[
    StructuredOutputMode::JsonObject,
    StructuredOutputMode::JsonSchema,
];

/// Strictness accepted by a JSON Schema structured-output capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonSchemaSupport {
    /// Accepts non-strict JSON Schema constraints only.
    NonStrictOnly,
    /// Accepts both non-strict JSON Schema and `strict: true` constraints.
    StrictSupported,
}

impl JsonSchemaSupport {
    /// Returns whether this strictness stays within another JSON Schema ceiling.
    const fn is_subset_of(self, upper: Self) -> bool {
        matches!(self, Self::NonStrictOnly) || matches!(upper, Self::StrictSupported)
    }

    /// Returns the strictness guaranteed by both profiles.
    const fn intersection(self, other: Self) -> Self {
        if matches!(self, Self::StrictSupported) && matches!(other, Self::StrictSupported) {
            Self::StrictSupported
        } else {
            Self::NonStrictOnly
        }
    }
}

/// Fine-grained function-tool capability profile.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FunctionToolCapabilities {
    /// Function-tool selection modes accepted by the operation.
    pub choice_modes: &'static [ToolChoiceMode],
    /// Whether `parallel_tool_calls: true` is accepted with function tools.
    pub parallel_calls: bool,
    /// Whether strict JSON Schema function parameters are accepted.
    pub strict_schema: bool,
}

impl FunctionToolCapabilities {
    /// Returns whether this profile is no broader than another function-tool profile.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        self.choice_modes
            .iter()
            .all(|mode| upper.choice_modes.contains(mode))
            && (!self.parallel_calls || upper.parallel_calls)
            && (!self.strict_schema || upper.strict_schema)
    }
}

/// Closed non-empty structured-output capability profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredOutputProfile {
    /// Accepts the JSON Object response format only.
    JsonObject,
    /// Accepts JSON Schema with the declared strictness support.
    JsonSchema(JsonSchemaSupport),
    /// Accepts JSON Object and JSON Schema with the declared strictness support.
    JsonObjectAndJsonSchema(JsonSchemaSupport),
}

impl StructuredOutputProfile {
    /// Returns supported modes in stable JSON Object then JSON Schema order.
    pub const fn modes(self) -> &'static [StructuredOutputMode] {
        match self {
            Self::JsonObject => JSON_OBJECT_MODE,
            Self::JsonSchema(_) => JSON_SCHEMA_MODE,
            Self::JsonObjectAndJsonSchema(_) => JSON_OBJECT_AND_SCHEMA_MODES,
        }
    }

    /// Returns whether this profile supports the requested structured-output mode.
    pub const fn supports(self, mode: StructuredOutputMode) -> bool {
        matches!(
            (self, mode),
            (
                Self::JsonObject | Self::JsonObjectAndJsonSchema(_),
                StructuredOutputMode::JsonObject
            ) | (
                Self::JsonSchema(_) | Self::JsonObjectAndJsonSchema(_),
                StructuredOutputMode::JsonSchema
            )
        )
    }

    /// Returns whether this profile accepts `strict: true` JSON Schema constraints.
    pub const fn supports_strict_schema(self) -> bool {
        matches!(
            self,
            Self::JsonSchema(JsonSchemaSupport::StrictSupported)
                | Self::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported)
        )
    }

    /// Returns whether this profile is no broader than another structured-output profile.
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        match (self, upper) {
            (Self::JsonObject, Self::JsonObject | Self::JsonObjectAndJsonSchema(_)) => true,
            (Self::JsonSchema(value), Self::JsonSchema(upper))
            | (Self::JsonSchema(value), Self::JsonObjectAndJsonSchema(upper)) => {
                value.is_subset_of(upper)
            }
            (Self::JsonObjectAndJsonSchema(value), Self::JsonObjectAndJsonSchema(upper)) => {
                value.is_subset_of(upper)
            }
            _ => false,
        }
    }

    /// Returns the closed profile guaranteed by both operands, or `None` for disjoint modes.
    pub(crate) const fn intersection(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::JsonObject, Self::JsonObject)
            | (Self::JsonObject, Self::JsonObjectAndJsonSchema(_))
            | (Self::JsonObjectAndJsonSchema(_), Self::JsonObject) => Some(Self::JsonObject),
            (Self::JsonSchema(value), Self::JsonSchema(other))
            | (Self::JsonSchema(value), Self::JsonObjectAndJsonSchema(other))
            | (Self::JsonObjectAndJsonSchema(value), Self::JsonSchema(other)) => {
                Some(Self::JsonSchema(value.intersection(other)))
            }
            (Self::JsonObjectAndJsonSchema(value), Self::JsonObjectAndJsonSchema(other)) => {
                Some(Self::JsonObjectAndJsonSchema(value.intersection(other)))
            }
            (Self::JsonObject, Self::JsonSchema(_)) | (Self::JsonSchema(_), Self::JsonObject) => {
                None
            }
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
fn image_input_is_subset_of(
    value: Option<ImageInputCapabilities>,
    upper: Option<ImageInputCapabilities>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
        (Some(_), None) => false,
    }
}

/// Shared generation-capability projection for Chat Completions and Responses.
///
/// This value is used only for common-protocol subset checks; static registrations must use
/// [`ChatCompletionsCapabilities`] or [`ResponsesCapabilities`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GenerationCapabilities {
    /// Whether incremental results can be returned over SSE.
    pub(crate) streaming: bool,
    /// Fine-grained function-tool capability profile.
    pub(crate) function_tools: Option<FunctionToolCapabilities>,
    /// Typed image input profile, or `None` when images are unsupported.
    pub(crate) image_input: Option<ImageInputCapabilities>,
    /// Fine-grained structured-output capability profile.
    pub(crate) structured_outputs: Option<StructuredOutputProfile>,
    /// Whether the request wire field `store: true` is supported.
    pub(crate) store: bool,
    /// Observable type of upstream reasoning output.
    pub(crate) reasoning_output: ReasoningOutput,
}

impl GenerationCapabilities {
    /// Returns whether the current capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        (!self.streaming || upper.streaming)
            && optional_function_tool_capabilities_is_subset_of(
                self.function_tools,
                upper.function_tools,
            )
            && image_input_is_subset_of(self.image_input, upper.image_input)
            && optional_structured_output_profile_is_subset_of(
                self.structured_outputs,
                upper.structured_outputs,
            )
            && (!self.store || upper.store)
            && self.reasoning_output.is_subset_of(upper.reasoning_output)
    }
}

/// Shared Chat Completions common fields parameterized by the audio contract layer.
///
/// Provider definitions use [`ProviderChatCompletionsCapabilities`], while concrete Target APIs
/// use [`ChatCompletionsCapabilities`]. The generic envelope prevents common fields from drifting
/// while the type parameter prevents a Provider ceiling from entering executable Route state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChatCompletionsProfile<A> {
    /// Whether Chat Completions streaming is supported.
    pub streaming: bool,
    /// Fine-grained function-tool capability profile, or `None` when tools are unsupported.
    pub function_tools: Option<FunctionToolCapabilities>,
    /// Typed `image_url` input profile, or `None` when images are unsupported.
    pub image_input: Option<ImageInputCapabilities>,
    /// Fine-grained structured-output profile, or `None` when structured output is unsupported.
    pub structured_outputs: Option<StructuredOutputProfile>,
    /// Whether the request wire field `store: true` is supported.
    pub store: bool,
    /// Observable type of upstream reasoning output.
    pub reasoning_output: ReasoningOutput,
    /// Whether `type: "custom"` tools are supported.
    pub custom_tool_calling: bool,
    /// Layer-specific Provider ceiling or executable audio profile.
    pub audio: A,
    /// Whether `file` input content parts are supported.
    pub file_input: bool,
    /// Whether `prediction` predicted outputs are supported.
    pub predicted_outputs: bool,
    /// Whether `web_search_options` is supported.
    pub web_search: bool,
    /// Whether prompt cache key/options/breakpoint semantics are supported.
    pub prompt_caching: bool,
    /// Whether request-level moderation configuration is supported.
    pub moderation: bool,
    /// Whether token log probabilities are supported.
    pub logprobs: bool,
    /// Whether multiple choices with `n > 1` are supported.
    pub multiple_choices: bool,
}

/// Provider-wide Chat Completions ceiling with an optional non-empty audio task set.
pub type ProviderChatCompletionsCapabilities = ChatCompletionsProfile<Option<ProviderAudioCeiling>>;

/// Concrete Target Chat Completions profile with at most one executable audio task.
pub type ChatCompletionsCapabilities = ChatCompletionsProfile<Option<ExecutableAudioProfile>>;

impl<A: Copy> ChatCompletionsProfile<A> {
    /// Extracts generation capabilities shared by Chat Completions and Responses.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
        }
    }

    /// Stops compilation when reserved fields are registered so they cannot become false runtime capabilities.
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || self.file_input
            || self.predicted_outputs
            || self.web_search
            || self.prompt_caching
            || self.moderation
            || self.logprobs
            || self.multiple_choices
        {
            unimplemented!("reserved Chat Completions capabilities are not implemented");
        }
        if self
            .function_tools
            .is_some_and(|profile| profile.choice_modes.is_empty())
        {
            panic!("invalid Chat Completions function-tool capability profile");
        }
    }
}

impl ChatCompletionsProfile<Option<ProviderAudioCeiling>> {
    /// Projects the shared Provider ceiling into a concrete Target profile and replaces only audio.
    pub const fn to_executable(
        self,
        audio: Option<ExecutableAudioProfile>,
    ) -> ChatCompletionsCapabilities {
        ChatCompletionsProfile {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
            custom_tool_calling: self.custom_tool_calling,
            audio,
            file_input: self.file_input,
            predicted_outputs: self.predicted_outputs,
            web_search: self.web_search,
            prompt_caching: self.prompt_caching,
            moderation: self.moderation,
            logprobs: self.logprobs,
            multiple_choices: self.multiple_choices,
        }
    }
}

impl ChatCompletionsProfile<Option<ExecutableAudioProfile>> {
    /// Returns whether this concrete Target profile stays within the Provider Chat ceiling.
    pub(crate) fn is_subset_of(self, upper: ProviderChatCompletionsCapabilities) -> bool {
        // Reject reserved fields before comparing trusted static contracts.
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // Compare common fields and require any executable audio profile to fit the same-task slot.
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
            && optional_executable_audio_is_subset_of(self.audio, upper.audio)
    }

    /// Returns whether the typed audio profile contains any input capability.
    pub const fn has_audio_input(self) -> bool {
        match self.audio {
            Some(audio) => audio.has_input(),
            None => false,
        }
    }

    /// Returns whether the typed audio profile contains generated-audio output.
    pub const fn has_audio_output(self) -> bool {
        match self.audio {
            Some(audio) => audio.has_output(),
            None => false,
        }
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

fn optional_function_tool_capabilities_is_subset_of(
    value: Option<FunctionToolCapabilities>,
    upper: Option<FunctionToolCapabilities>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

fn optional_structured_output_profile_is_subset_of(
    value: Option<StructuredOutputProfile>,
    upper: Option<StructuredOutputProfile>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

/// Whether a Responses API accepts persistent response storage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StorageSupport {
    /// The executable API does not accept `store: true`.
    #[default]
    Unsupported,
    /// The executable API accepts `store: true`.
    Supported,
}

impl StorageSupport {
    /// Returns whether persistent response storage is supported.
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// Closed Target-affinity contract for one executable Responses API.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResponsesAffinity {
    /// Requests carry no Provider state that binds execution to this Target.
    #[default]
    Unbound,
    /// Provider state is Target-bound, but continuation by response ID is unsupported.
    TargetBound,
    /// Provider-issued response IDs bind continuation to this Target and credential context.
    TargetBoundContinuation,
}

/// Executable storage and affinity state owned by one concrete Responses Target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutableResponsesState {
    storage: StorageSupport,
    affinity: ResponsesAffinity,
}

impl ExecutableResponsesState {
    /// Creates one executable Responses state from independent storage and closed affinity facts.
    pub const fn new(storage: StorageSupport, affinity: ResponsesAffinity) -> Self {
        Self { storage, affinity }
    }

    /// Returns the persistent-storage support carried by this executable state.
    pub const fn storage(self) -> StorageSupport {
        self.storage
    }

    /// Returns the closed Target-affinity variant carried by this executable state.
    pub const fn affinity(self) -> ResponsesAffinity {
        self.affinity
    }

    /// Returns whether this executable Responses API accepts `store: true`.
    pub const fn supports_store(self) -> bool {
        self.storage.is_supported()
    }

    /// Returns whether this executable Responses API accepts `previous_response_id`.
    pub const fn supports_previous_response_id(self) -> bool {
        matches!(self.affinity, ResponsesAffinity::TargetBoundContinuation)
    }

    /// Returns whether Provider state is bound to the concrete Upstream Target.
    pub const fn is_target_bound(self) -> bool {
        matches!(
            self.affinity,
            ResponsesAffinity::TargetBound | ResponsesAffinity::TargetBoundContinuation
        )
    }

    /// Returns whether continuation safety requires one enabled credential-pool member.
    pub const fn requires_single_credential_member(self) -> bool {
        self.supports_previous_response_id()
    }

    /// Returns whether this executable state stays within one Provider state ceiling.
    const fn is_subset_of(self, upper: ProviderResponsesStateCeiling) -> bool {
        (!self.supports_store() || upper.supports_store())
            && (!self.supports_previous_response_id() || upper.supports_previous_response_id())
    }
}

/// Provider-wide upper bound for the two independent Responses state capabilities.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProviderResponsesStateCeiling {
    /// Neither storage nor continuation is supported.
    #[default]
    Stateless,
    /// Persistent response storage is supported without continuation.
    Storage,
    /// Response-ID continuation is supported without persistent storage.
    Continuation,
    /// Both persistent storage and response-ID continuation are supported.
    StorageAndContinuation,
}

impl ProviderResponsesStateCeiling {
    /// Returns whether the Provider ceiling permits `store: true`.
    pub const fn supports_store(self) -> bool {
        matches!(self, Self::Storage | Self::StorageAndContinuation)
    }

    /// Returns whether the Provider ceiling permits response-ID continuation.
    pub const fn supports_previous_response_id(self) -> bool {
        matches!(self, Self::Continuation | Self::StorageAndContinuation)
    }
}

/// Shared Responses Create fields parameterized by the state contract layer.
///
/// Provider definitions use [`ProviderResponsesCapabilities`], while concrete Target APIs use
/// [`ResponsesCapabilities`]. Other endpoints such as resource retrieve/cancel/delete remain
/// outside this structure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResponsesProfile<S> {
    /// Whether Responses streaming is supported.
    pub streaming: bool,
    /// Fine-grained function-tool capability profile, or `None` when tools are unsupported.
    pub function_tools: Option<FunctionToolCapabilities>,
    /// Typed `input_image` profile, or `None` when images are unsupported.
    pub image_input: Option<ImageInputCapabilities>,
    /// Fine-grained structured-output profile, or `None` when structured output is unsupported.
    pub structured_outputs: Option<StructuredOutputProfile>,
    /// Layer-specific Provider ceiling or executable Responses state.
    pub state: S,
    /// Whether background responses are supported.
    pub background: bool,
    /// Observable type of upstream reasoning output.
    pub reasoning_output: ReasoningOutput,
    /// Whether `type: "custom"` tools are supported.
    pub custom_tool_calling: bool,
    /// Declared OpenAI-hosted tool kinds.
    pub hosted_tools: &'static [HostedToolKind],
    /// Whether file input items/content parts are supported.
    pub file_input: bool,
    /// Whether persistent `conversation` state is supported.
    pub conversation: bool,
    /// Whether `prompt` template references are supported.
    pub prompt_templates: bool,
    /// Whether prompt cache key/options/breakpoint semantics are supported.
    pub prompt_caching: bool,
    /// Whether `context_management` is supported.
    pub context_management: bool,
    /// Declared additional output kinds supported by `include`.
    pub include: &'static [ResponseInclude],
    /// Whether request-level moderation configuration is supported.
    pub moderation: bool,
    /// Whether message output-text log probabilities are supported.
    pub logprobs: bool,
}

/// Provider-wide Responses ceiling without Target ownership state.
pub type ProviderResponsesCapabilities = ResponsesProfile<ProviderResponsesStateCeiling>;

/// Concrete executable Responses profile with one closed Target state.
pub type ResponsesCapabilities = ResponsesProfile<ExecutableResponsesState>;

impl ProviderResponsesCapabilities {
    /// Returns the complete Provider Responses state ceiling.
    pub const fn state_ceiling(self) -> ProviderResponsesStateCeiling {
        self.state
    }

    /// Returns whether the Provider ceiling permits persistent response storage.
    pub const fn supports_store(self) -> bool {
        self.state.supports_store()
    }

    /// Returns whether the Provider ceiling permits response-ID continuation.
    pub const fn supports_previous_response_id(self) -> bool {
        self.state.supports_previous_response_id()
    }

    /// Projects the Provider ceiling into one concrete Target profile and replaces only state.
    pub const fn to_executable(self, state: ExecutableResponsesState) -> ResponsesCapabilities {
        ResponsesProfile {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            state,
            background: self.background,
            reasoning_output: self.reasoning_output,
            custom_tool_calling: self.custom_tool_calling,
            hosted_tools: self.hosted_tools,
            file_input: self.file_input,
            conversation: self.conversation,
            prompt_templates: self.prompt_templates,
            prompt_caching: self.prompt_caching,
            context_management: self.context_management,
            include: self.include,
            moderation: self.moderation,
            logprobs: self.logprobs,
        }
    }

    /// Extracts common generation capabilities from the Provider ceiling.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.state.supports_store(),
            reasoning_output: self.reasoning_output,
        }
    }
}

impl ResponsesCapabilities {
    /// Extracts endpoint capabilities shared by Responses and Chat.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.state.supports_store(),
            reasoning_output: self.reasoning_output,
        }
    }

    /// Returns the complete executable Responses state.
    pub const fn state(self) -> ExecutableResponsesState {
        self.state
    }

    /// Returns whether this executable API accepts persistent response storage.
    pub const fn supports_store(self) -> bool {
        self.state.supports_store()
    }

    /// Returns whether this executable API accepts response-ID continuation.
    pub const fn supports_previous_response_id(self) -> bool {
        self.state.supports_previous_response_id()
    }

    /// Returns whether Provider state is bound to the concrete Upstream Target.
    pub const fn is_target_bound(self) -> bool {
        self.state.is_target_bound()
    }

    /// Returns whether continuation safety requires one enabled credential-pool member.
    pub const fn requires_single_credential_member(self) -> bool {
        self.state.requires_single_credential_member()
    }

    /// Returns whether the current Responses capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: ProviderResponsesCapabilities) -> bool {
        // Prevent reserved fields from entering the static capability contract before request handling exists.
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // Compare implemented common capabilities and Responses state capabilities.
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
            && self.state.is_subset_of(upper.state)
            && (!self.background || upper.background)
    }
}

impl<S: Copy> ResponsesProfile<S> {
    /// Stops compilation when reserved fields are registered so they cannot become false runtime capabilities.
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || !self.hosted_tools.is_empty()
            || self.file_input
            || self.conversation
            || self.prompt_templates
            || self.prompt_caching
            || self.context_management
            || !self.include.is_empty()
            || self.moderation
            || self.logprobs
        {
            unimplemented!("reserved Responses capabilities are not implemented");
        }
        if self
            .function_tools
            .is_some_and(|profile| profile.choice_modes.is_empty())
        {
            panic!("invalid Responses function-tool capability profile");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputLimits, AudioInputSource,
        AudioUnderstandingProfile, ChatCompletionsCapabilities, ExecutableAudioProfile,
        FunctionToolCapabilities, GeneratedAudioCapabilities, ImageDetail, ImageDetailPolicy,
        ImageDetailProfile, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
        InlineImageInputLimits, InlineImageInputProfile, JsonAudioDelivery, JsonAudioFraming,
        JsonSchemaSupport, PresetVoiceCapabilities, ProviderAudioCeiling,
        ProviderChatCompletionsCapabilities, RemoteImageInputLimits, SpeechRecognitionProfile,
        SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming, StructuredOutputMode,
        StructuredOutputProfile, ToolChoiceMode, VoiceCloneProfile, VoiceDesignProfile,
    };

    const ALL_TOOL_CHOICES: &[ToolChoiceMode] = super::ALL_TOOL_CHOICE_MODES;
    const COMMON_TOOL_CHOICES: &[ToolChoiceMode] = &[ToolChoiceMode::None, ToolChoiceMode::Auto];

    const STRUCTURED_OUTPUT_PROFILES: [StructuredOutputProfile; 5] = [
        StructuredOutputProfile::JsonObject,
        StructuredOutputProfile::JsonSchema(JsonSchemaSupport::NonStrictOnly),
        StructuredOutputProfile::JsonSchema(JsonSchemaSupport::StrictSupported),
        StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::NonStrictOnly),
        StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported),
    ];

    #[test]
    fn structured_output_profiles_expose_stable_modes_and_strictness() {
        let [object, schema, strict_schema, combined, strict_combined] = STRUCTURED_OUTPUT_PROFILES;

        assert_eq!(object.modes(), &[StructuredOutputMode::JsonObject]);
        assert_eq!(schema.modes(), &[StructuredOutputMode::JsonSchema]);
        assert_eq!(
            combined.modes(),
            &[
                StructuredOutputMode::JsonObject,
                StructuredOutputMode::JsonSchema,
            ]
        );
        assert!(object.supports(StructuredOutputMode::JsonObject));
        assert!(!object.supports(StructuredOutputMode::JsonSchema));
        assert!(!schema.supports(StructuredOutputMode::JsonObject));
        assert!(schema.supports(StructuredOutputMode::JsonSchema));
        assert!(combined.supports(StructuredOutputMode::JsonObject));
        assert!(combined.supports(StructuredOutputMode::JsonSchema));
        assert!(!object.supports_strict_schema());
        assert!(!schema.supports_strict_schema());
        assert!(strict_schema.supports_strict_schema());
        assert!(!combined.supports_strict_schema());
        assert!(strict_combined.supports_strict_schema());
    }

    #[test]
    fn structured_output_profiles_enforce_the_complete_subset_lattice() {
        let expected = [
            [true, false, false, true, true],
            [false, true, true, true, true],
            [false, false, true, false, true],
            [false, false, false, true, true],
            [false, false, false, false, true],
        ];

        for (value_index, value) in STRUCTURED_OUTPUT_PROFILES.into_iter().enumerate() {
            for (upper_index, upper) in STRUCTURED_OUTPUT_PROFILES.into_iter().enumerate() {
                assert_eq!(
                    value.is_subset_of(upper),
                    expected[value_index][upper_index],
                    "unexpected subset result for {value:?} <= {upper:?}"
                );
            }
        }
    }

    #[test]
    fn structured_output_profiles_enforce_the_complete_intersection_lattice() {
        let [object, schema, strict_schema, combined, strict_combined] = STRUCTURED_OUTPUT_PROFILES;
        let expected = [
            [Some(object), None, None, Some(object), Some(object)],
            [None, Some(schema), Some(schema), Some(schema), Some(schema)],
            [
                None,
                Some(schema),
                Some(strict_schema),
                Some(schema),
                Some(strict_schema),
            ],
            [
                Some(object),
                Some(schema),
                Some(schema),
                Some(combined),
                Some(combined),
            ],
            [
                Some(object),
                Some(schema),
                Some(strict_schema),
                Some(combined),
                Some(strict_combined),
            ],
        ];

        for (value_index, value) in STRUCTURED_OUTPUT_PROFILES.into_iter().enumerate() {
            for (other_index, other) in STRUCTURED_OUTPUT_PROFILES.into_iter().enumerate() {
                assert_eq!(
                    value.intersection(other),
                    expected[value_index][other_index],
                    "unexpected intersection for {value:?} and {other:?}"
                );
            }
        }
    }

    #[test]
    fn image_source_and_detail_unions_enforce_the_complete_subset_lattice() {
        let narrow_data = InlineImageInputProfile::new(
            &[ImageMediaType::Png],
            InlineImageInputLimits::new(1_024, 768, 2_048, 1_536),
        );
        let broad_data = InlineImageInputProfile::new(
            &[ImageMediaType::Png, ImageMediaType::Jpeg],
            InlineImageInputLimits::new(2_048, 1_536, 4_096, 3_072),
        );
        let narrow_remote = RemoteImageInputLimits::new(1_024);
        let broad_remote = RemoteImageInputLimits::new(2_048);
        let narrow_detail = ImageDetailPolicy::Explicit(ImageDetailProfile::new(
            Some(ImageDetail::Auto),
            &[ImageDetail::Low],
        ));
        let broad_detail = ImageDetailPolicy::Explicit(ImageDetailProfile::new(
            Some(ImageDetail::Auto),
            &[ImageDetail::Low, ImageDetail::High],
        ));
        let broad = ImageInputCapabilities::new(
            2,
            ImageSourceCapabilities::RemoteUrlAndDataUrl {
                remote: broad_remote,
                data: broad_data,
            },
            broad_detail,
        );

        // Allow either single source or a narrower complete pair under the combined ceiling.
        for value in [
            ImageInputCapabilities::new(
                1,
                ImageSourceCapabilities::RemoteUrl(narrow_remote),
                ImageDetailPolicy::OmittedOnly {
                    default: Some(ImageDetail::Auto),
                },
            ),
            ImageInputCapabilities::new(
                2,
                ImageSourceCapabilities::DataUrl(narrow_data),
                narrow_detail,
            ),
            ImageInputCapabilities::new(
                2,
                ImageSourceCapabilities::RemoteUrlAndDataUrl {
                    remote: narrow_remote,
                    data: narrow_data,
                },
                narrow_detail,
            ),
        ] {
            assert!(value.is_subset_of(broad));
        }

        // Reject cross-source substitution, combined-to-single narrowing, and default drift.
        let remote_only = ImageInputCapabilities::new(
            2,
            ImageSourceCapabilities::RemoteUrl(broad_remote),
            broad_detail,
        );
        let data_only = ImageInputCapabilities::new(
            2,
            ImageSourceCapabilities::DataUrl(broad_data),
            broad_detail,
        );
        assert!(!data_only.is_subset_of(remote_only));
        assert!(!broad.is_subset_of(remote_only));
        assert!(!broad.is_subset_of(data_only));
        assert!(
            !ImageDetailPolicy::Explicit(ImageDetailProfile::new(None, &[ImageDetail::Low]))
                .is_subset_of(broad_detail)
        );
        assert!(!broad_detail.is_subset_of(ImageDetailPolicy::OmittedOnly {
            default: Some(ImageDetail::Auto)
        }));
    }

    #[test]
    fn remote_image_profile_narrows_the_mimo_combined_provider_ceiling() {
        let provider = crate::providers::mimo::DEFINITION
            .contract()
            .capabilities()
            .chat_completions
            .expect("MiMo must expose Chat Completions");
        let remote = ImageInputCapabilities::new(
            2,
            ImageSourceCapabilities::RemoteUrl(RemoteImageInputLimits::new(1_024)),
            ImageDetailPolicy::OmittedOnly { default: None },
        );
        let mut target = provider.to_executable(None);
        target.image_input = Some(remote);

        assert!(remote.is_subset_of(provider.image_input.unwrap()));
        assert!(target.is_subset_of(provider));
    }

    #[test]
    #[should_panic(
        expected = "remote image URL length limit must allow the nine-byte URL https://a"
    )]
    fn remote_image_input_rejects_a_limit_below_the_shortest_absolute_https_url() {
        let _ = RemoteImageInputLimits::new(8);
    }

    #[test]
    #[should_panic(
        expected = "inline image encoded-byte limit must allow one four-byte Base64 quantum"
    )]
    fn inline_image_input_rejects_an_encoded_limit_below_one_base64_quantum() {
        let _ = InlineImageInputLimits::new(3, 1, 3, 1);
    }

    #[test]
    fn image_input_limits_accept_the_smallest_wire_reachable_values() {
        let remote = RemoteImageInputLimits::new(9);
        let inline = InlineImageInputLimits::new(4, 1, 4, 1);

        assert_eq!(remote.max_url_length(), 9);
        assert_eq!(inline.max_inline_encoded_bytes(), 4);
        assert_eq!(inline.max_inline_decoded_bytes(), 1);
        assert_eq!(inline.max_total_inline_encoded_bytes(), 4);
        assert_eq!(inline.max_total_inline_decoded_bytes(), 1);
    }

    #[test]
    #[should_panic(expected = "inline image media types must not be empty")]
    fn inline_image_input_rejects_an_empty_media_set() {
        let _ = InlineImageInputProfile::new(&[], InlineImageInputLimits::new(4, 1, 4, 1));
    }

    #[test]
    #[should_panic(expected = "image media types must not contain duplicates")]
    fn inline_image_input_rejects_duplicate_media_types() {
        let _ = InlineImageInputProfile::new(
            &[ImageMediaType::Png, ImageMediaType::Png],
            InlineImageInputLimits::new(4, 1, 4, 1),
        );
    }

    #[test]
    #[should_panic(expected = "total encoded-byte limit must cover one inline image")]
    fn inline_image_input_rejects_a_total_smaller_than_one_item() {
        let _ = InlineImageInputLimits::new(4, 1, 3, 1);
    }

    #[test]
    #[should_panic(expected = "image input max_parts must be positive")]
    fn image_input_rejects_zero_max_parts() {
        let _ = ImageInputCapabilities::new(
            0,
            ImageSourceCapabilities::RemoteUrl(RemoteImageInputLimits::new(9)),
            ImageDetailPolicy::OmittedOnly { default: None },
        );
    }

    #[test]
    #[should_panic(expected = "total encoded-byte limit exceeds the image per-part capacity")]
    fn image_input_rejects_an_unreachable_cumulative_budget() {
        let data = InlineImageInputProfile::new(
            &[ImageMediaType::Png],
            InlineImageInputLimits::new(4, 1, 9, 1),
        );
        let _ = ImageInputCapabilities::new(
            2,
            ImageSourceCapabilities::DataUrl(data),
            ImageDetailPolicy::OmittedOnly { default: None },
        );
    }

    #[test]
    #[should_panic(expected = "explicit image details must not be empty")]
    fn explicit_image_detail_rejects_an_empty_domain() {
        let _ = ImageDetailProfile::new(Some(ImageDetail::Auto), &[]);
    }

    #[test]
    #[should_panic(expected = "image details must not contain duplicates")]
    fn explicit_image_detail_rejects_duplicate_values() {
        let _ = ImageDetailProfile::new(None, &[ImageDetail::Low, ImageDetail::Low]);
    }

    #[test]
    fn typed_generation_profiles_intersect_without_boolean_union() {
        let provider_ceiling = ProviderChatCompletionsCapabilities {
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICES,
                parallel_calls: true,
                strict_schema: true,
            }),
            structured_outputs: Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::StrictSupported,
            )),
            ..Default::default()
        };
        let public_contract = ChatCompletionsCapabilities {
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: COMMON_TOOL_CHOICES,
                parallel_calls: false,
                strict_schema: false,
            }),
            structured_outputs: Some(StructuredOutputProfile::JsonObject),
            ..Default::default()
        };

        assert!(public_contract.is_subset_of(provider_ceiling));
        let narrower_ceiling = ProviderChatCompletionsCapabilities {
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: COMMON_TOOL_CHOICES,
                parallel_calls: false,
                strict_schema: false,
            }),
            structured_outputs: Some(StructuredOutputProfile::JsonObject),
            ..Default::default()
        };
        let elevated_contract = ChatCompletionsCapabilities {
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICES,
                parallel_calls: true,
                strict_schema: true,
            }),
            structured_outputs: Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::StrictSupported,
            )),
            ..Default::default()
        };
        assert!(!elevated_contract.is_subset_of(narrower_ceiling));
    }

    #[test]
    fn audio_presence_is_derived_from_typed_profiles() {
        let generated = generated_audio(&[AudioFormat::Wav], &[AudioFormat::Pcm16]);
        let audio = ExecutableAudioProfile::SpeechSynthesis(SpeechSynthesisProfile::new(
            generated,
            PresetVoiceCapabilities::new(&["test_voice"]),
        ));
        let capabilities = ChatCompletionsCapabilities {
            audio: Some(audio),
            ..Default::default()
        };

        assert!(!capabilities.has_audio_input());
        assert!(capabilities.has_audio_output());
    }

    #[test]
    fn provider_audio_ceiling_contains_only_same_variant_subsets() {
        let broad_input = audio_input(&[AudioFormat::Wav, AudioFormat::Mp3], 2);
        let narrow_input = audio_input(&[AudioFormat::Wav], 1);
        let ceiling = ProviderAudioCeiling::new(ExecutableAudioProfile::SpeechRecognition(
            SpeechRecognitionProfile::new(
                broad_input,
                &[AsrLanguage::Auto, AsrLanguage::Zh, AsrLanguage::En],
            ),
        ))
        .with(ExecutableAudioProfile::SpeechSynthesis(
            SpeechSynthesisProfile::new(
                generated_audio(&[AudioFormat::Wav, AudioFormat::Mp3], &[AudioFormat::Pcm16]),
                PresetVoiceCapabilities::new(&["test_voice"]),
            ),
        ));
        let provider = ProviderChatCompletionsCapabilities {
            audio: Some(ceiling),
            ..Default::default()
        };
        let asr = provider.to_executable(Some(ExecutableAudioProfile::SpeechRecognition(
            SpeechRecognitionProfile::new(narrow_input, &[AsrLanguage::Auto]),
        )));
        let wrong_variant = provider.to_executable(Some(ExecutableAudioProfile::VoiceDesign(
            super::VoiceDesignProfile::new(generated_audio(
                &[AudioFormat::Wav],
                &[AudioFormat::Pcm16],
            )),
        )));

        assert!(asr.is_subset_of(provider));
        assert!(!wrong_variant.is_subset_of(provider));
    }

    #[test]
    fn provider_audio_ceiling_accepts_all_five_complete_task_variants() {
        let input = audio_input(&[AudioFormat::Wav], 1);
        let generated = generated_audio(&[AudioFormat::Wav], &[AudioFormat::Pcm16]);
        let profiles = [
            ExecutableAudioProfile::AudioUnderstanding(AudioUnderstandingProfile::new(input)),
            ExecutableAudioProfile::SpeechRecognition(SpeechRecognitionProfile::new(
                input,
                &[AsrLanguage::Auto],
            )),
            ExecutableAudioProfile::SpeechSynthesis(SpeechSynthesisProfile::new(
                generated,
                PresetVoiceCapabilities::new(&["test_voice"]),
            )),
            ExecutableAudioProfile::VoiceDesign(VoiceDesignProfile::new(generated)),
            ExecutableAudioProfile::VoiceClone(VoiceCloneProfile::new(input, generated)),
        ];
        let ceiling = ProviderAudioCeiling::new(profiles[0])
            .with(profiles[1])
            .with(profiles[2])
            .with(profiles[3])
            .with(profiles[4]);
        let provider = ProviderChatCompletionsCapabilities {
            audio: Some(ceiling),
            ..Default::default()
        };

        for profile in profiles {
            assert!(provider.to_executable(Some(profile)).is_subset_of(provider));
        }
    }

    #[test]
    #[should_panic(expected = "duplicate speech-recognition Provider ceiling")]
    fn provider_audio_ceiling_rejects_duplicate_task_profiles() {
        let profile = ExecutableAudioProfile::SpeechRecognition(SpeechRecognitionProfile::new(
            audio_input(&[AudioFormat::Wav], 1),
            &[AsrLanguage::Auto],
        ));
        let _ = ProviderAudioCeiling::new(profile).with(profile);
    }

    #[test]
    #[should_panic(expected = "JSON audio formats must not be empty")]
    fn generated_audio_rejects_empty_json_delivery() {
        let _ = JsonAudioDelivery::new(&[], 1, 1, JsonAudioFraming::ChatMessageAudioData);
    }

    #[test]
    #[should_panic(expected = "SSE audio decoded budget must be positive")]
    fn generated_audio_rejects_zero_sse_budget() {
        let _ = SseAudioDelivery::new(
            &[AudioFormat::Pcm16],
            0,
            SseAudioFraming::ChatDeltaAudioData,
        );
    }

    #[test]
    #[should_panic(expected = "audio input sources must not be empty")]
    fn audio_input_rejects_an_empty_source_set() {
        let _ = AudioInputCapabilities::new(
            &[],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 0, 1, 1, 1, 1),
        );
    }

    #[test]
    #[should_panic(expected = "audio input sources must not contain duplicates")]
    fn audio_input_rejects_duplicate_sources() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl, AudioInputSource::DataUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 0, 1, 1, 1, 1),
        );
    }

    #[test]
    #[should_panic(expected = "audio formats must not contain duplicates")]
    fn audio_input_rejects_duplicate_formats() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl],
            &[AudioFormat::Wav, AudioFormat::Wav],
            AudioInputLimits::new(1, 0, 1, 1, 1, 1),
        );
    }

    #[test]
    #[should_panic(expected = "audio input max_parts must be positive")]
    fn audio_input_rejects_zero_max_parts() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(0, 0, 1, 1, 1, 1),
        );
    }

    #[test]
    #[should_panic(expected = "remote audio input requires a positive URL length limit")]
    fn audio_input_requires_a_remote_url_limit() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::RemoteUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 0, 0, 0, 0, 0),
        );
    }

    #[test]
    #[should_panic(expected = "inline audio input requires inline byte limits")]
    fn audio_input_requires_inline_limits() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 0, 0, 0, 0, 0),
        );
    }

    #[test]
    #[should_panic(expected = "audio input without remote URLs must have a zero URL length limit")]
    fn audio_input_rejects_url_limits_without_remote_sources() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 128, 1, 1, 1, 1),
        );
    }

    #[test]
    #[should_panic(expected = "audio input without inline sources must have zero inline limits")]
    fn audio_input_rejects_inline_limits_without_inline_sources() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::RemoteUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 128, 1, 1, 1, 1),
        );
    }

    #[test]
    #[should_panic(expected = "total encoded-byte limit exceeds the declared per-part capacity")]
    fn audio_input_rejects_unreachable_aggregate_inline_limits() {
        let _ = AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl],
            &[AudioFormat::Wav],
            AudioInputLimits::new(1, 0, 1, 1, 2, 1),
        );
    }

    #[test]
    #[should_panic(expected = "audio formats must not contain duplicates")]
    fn generated_audio_rejects_duplicate_delivery_formats() {
        let _ = JsonAudioDelivery::new(
            &[AudioFormat::Wav, AudioFormat::Wav],
            1,
            1,
            JsonAudioFraming::ChatMessageAudioData,
        );
    }

    #[test]
    #[should_panic(expected = "audio formats must not contain duplicates")]
    fn generated_audio_rejects_duplicate_streaming_formats() {
        let _ = SseAudioDelivery::new(
            &[AudioFormat::Pcm16, AudioFormat::Pcm16],
            1,
            SseAudioFraming::ChatDeltaAudioData,
        );
    }

    #[test]
    #[should_panic(expected = "ASR languages must not contain duplicates")]
    fn speech_recognition_rejects_duplicate_languages() {
        let _ = SpeechRecognitionProfile::new(
            audio_input(&[AudioFormat::Wav], 1),
            &[AsrLanguage::Auto, AsrLanguage::Auto],
        );
    }

    #[test]
    #[should_panic(expected = "preset voices must not contain duplicates")]
    fn speech_synthesis_rejects_duplicate_preset_voices() {
        let _ = PresetVoiceCapabilities::new(&["test_voice", "test_voice"]);
    }

    fn audio_input(formats: &'static [AudioFormat], max_parts: u32) -> AudioInputCapabilities {
        AudioInputCapabilities::new(
            &[AudioInputSource::DataUrl],
            formats,
            AudioInputLimits::new(max_parts, 0, 1024, 768, 1024, 768),
        )
    }

    fn generated_audio(
        json_formats: &'static [AudioFormat],
        sse_formats: &'static [AudioFormat],
    ) -> GeneratedAudioCapabilities {
        GeneratedAudioCapabilities::new(
            JsonAudioDelivery::new(
                json_formats,
                2048,
                1536,
                JsonAudioFraming::ChatMessageAudioData,
            ),
            SseAudioDelivery::new(sse_formats, 4096, SseAudioFraming::ChatDeltaAudioData),
        )
    }
}
