//! Public Model-owned media execution algebra and downstream-safe projections.

use super::*;

///
/// The private source and detail unions are the execution contract. Serialization projects this
/// contract into the established flat Models extension without turning zero values into state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageInputInterfaceCapabilities {
    pub(super) max_parts: u32,
    sources: OwnedImageSourceCapabilities,
    detail: OwnedImageDetailPolicy,
}

/// Source-specific payload owned by one compiled Public Model interface.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnedImageSourceCapabilities {
    /// Remote URLs with their applicable URL byte limit.
    Remote(OwnedRemoteImageInputLimits),
    /// Inline data URLs with media types and inline byte limits.
    Inline(OwnedInlineImageInputProfile),
    /// Both independently complete source payloads.
    RemoteAndInline {
        remote: OwnedRemoteImageInputLimits,
        data: OwnedInlineImageInputProfile,
    },
}

/// Owned remote-URL limits after conservative Public Model aggregation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedRemoteImageInputLimits {
    pub(super) max_url_length: u32,
}

/// Owned inline data-URL profile after media-type intersection.
#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedInlineImageInputProfile {
    pub(super) media_types: Vec<ImageMediaType>,
    pub(super) limits: OwnedInlineImageInputLimits,
}

/// Owned inline byte limits after cross-field reachability narrowing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedInlineImageInputLimits {
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) max_total_inline_encoded_bytes: u32,
    pub(super) max_total_inline_decoded_bytes: u32,
}

/// Closed omitted-versus-explicit detail policy owned by request preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnedImageDetailPolicy {
    /// Clients may omit `detail`, but cannot submit an explicit value.
    OmittedOnly { default: Option<ImageDetail> },
    /// Clients may omit `detail` and may submit one value from the explicit set.
    Explicit {
        default: Option<ImageDetail>,
        allowed: Vec<ImageDetail>,
    },
}

/// Borrowed-free wire projection preserving the established Models JSON layout.
#[derive(Serialize)]
struct ImageInputInterfaceCapabilitiesWire {
    pub(super) sources: Vec<ImageInputSource>,
    pub(super) media_types: Vec<ImageMediaType>,
    pub(super) detail: ImageDetailCapabilities,
    pub(super) limits: ImageInputLimits,
}

/// Flat Models v1 wire projection of source-owned audio limits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AudioInputLimits {
    pub(super) max_parts: u32,
    pub(super) max_url_length: u32,
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) max_total_inline_encoded_bytes: u32,
    pub(super) max_total_inline_decoded_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedRemoteAudioInputProfile {
    pub(super) formats: Vec<AudioFormat>,
    pub(super) max_url_length: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedInlineAudioInputProfile {
    pub(super) formats: Vec<AudioFormat>,
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) max_total_inline_encoded_bytes: u32,
    pub(super) max_total_inline_decoded_bytes: u32,
}

/// Typed source-owned audio contract for one Native Chat interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioInputInterfaceCapabilities {
    pub(super) max_parts: u32,
    remote_url: Option<OwnedRemoteAudioInputProfile>,
    data_url: Option<OwnedInlineAudioInputProfile>,
    base64: Option<OwnedInlineAudioInputProfile>,
}

#[derive(Serialize)]
struct AudioInputInterfaceCapabilitiesWire {
    pub(super) sources: Vec<AudioInputSource>,
    pub(super) formats: Vec<AudioFormat>,
    pub(super) limits: AudioInputLimits,
}

impl OwnedRemoteAudioInputProfile {
    fn from_capabilities(value: RemoteAudioInputProfile) -> Self {
        Self {
            formats: value.formats().to_vec(),
            max_url_length: value.max_url_length(),
        }
    }

    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let mut result = values.first()?.to_owned().clone();
        result
            .formats
            .retain(|format| values.iter().all(|value| value.formats.contains(format)));
        result.max_url_length = values.iter().map(|value| value.max_url_length).min()?;
        (!result.formats.is_empty()).then_some(result)
    }
}

impl OwnedInlineAudioInputProfile {
    fn from_capabilities(value: InlineAudioInputProfile) -> Self {
        let limits = value.limits();
        Self {
            formats: value.formats().to_vec(),
            max_inline_encoded_bytes: limits.max_inline_encoded_bytes(),
            max_inline_decoded_bytes: limits.max_inline_decoded_bytes(),
            max_total_inline_encoded_bytes: limits.max_total_inline_encoded_bytes(),
            max_total_inline_decoded_bytes: limits.max_total_inline_decoded_bytes(),
        }
    }

    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let mut result = values.first()?.to_owned().clone();
        result
            .formats
            .retain(|format| values.iter().all(|value| value.formats.contains(format)));
        result.max_inline_encoded_bytes = values
            .iter()
            .map(|value| value.max_inline_encoded_bytes)
            .min()?;
        result.max_inline_decoded_bytes = values
            .iter()
            .map(|value| value.max_inline_decoded_bytes)
            .min()?;
        result.max_total_inline_encoded_bytes = values
            .iter()
            .map(|value| value.max_total_inline_encoded_bytes)
            .min()?;
        result.max_total_inline_decoded_bytes = values
            .iter()
            .map(|value| value.max_total_inline_decoded_bytes)
            .min()?;
        (!result.formats.is_empty()).then_some(result)
    }

    fn narrow_to_parts(&mut self, max_parts: u32) {
        self.max_total_inline_encoded_bytes = self
            .max_total_inline_encoded_bytes
            .min(self.max_inline_encoded_bytes.saturating_mul(max_parts));
        self.max_total_inline_decoded_bytes = self
            .max_total_inline_decoded_bytes
            .min(self.max_inline_decoded_bytes.saturating_mul(max_parts));
    }
}

impl AudioInputInterfaceCapabilities {
    /// Converts one static audio input profile into a downstream-safe owned contract.
    fn from_capabilities(value: AudioInputCapabilities) -> Self {
        Self {
            max_parts: value.max_parts(),
            remote_url: value
                .remote_url()
                .map(OwnedRemoteAudioInputProfile::from_capabilities),
            data_url: value
                .data_url()
                .map(OwnedInlineAudioInputProfile::from_capabilities),
            base64: value
                .base64()
                .map(OwnedInlineAudioInputProfile::from_capabilities),
        }
    }

    /// Intersects all candidate profiles without exposing Route or Provider identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let max_parts = values.iter().map(|value| value.max_parts).min()?;
        let remote_url = OwnedRemoteAudioInputProfile::intersection(
            values.iter().map(|value| value.remote_url.as_ref()),
        );
        let mut data_url = OwnedInlineAudioInputProfile::intersection(
            values.iter().map(|value| value.data_url.as_ref()),
        );
        let mut base64 = OwnedInlineAudioInputProfile::intersection(
            values.iter().map(|value| value.base64.as_ref()),
        );
        if let Some(profile) = data_url.as_mut() {
            profile.narrow_to_parts(max_parts);
        }
        if let Some(profile) = base64.as_mut() {
            profile.narrow_to_parts(max_parts);
        }
        (remote_url.is_some() || data_url.is_some() || base64.is_some()).then_some(Self {
            max_parts,
            remote_url,
            data_url,
            base64,
        })
    }

    /// Returns whether this interface accepts one audio source.
    pub(crate) fn supports_source(&self, source: AudioInputSource) -> bool {
        match source {
            AudioInputSource::RemoteUrl => self.remote_url.is_some(),
            AudioInputSource::DataUrl => self.data_url.is_some(),
            AudioInputSource::Base64 => self.base64.is_some(),
        }
    }

    /// Returns whether one source accepts one audio format.
    pub(crate) fn supports_format(&self, source: AudioInputSource, format: AudioFormat) -> bool {
        match source {
            AudioInputSource::RemoteUrl => self
                .remote_url
                .as_ref()
                .is_some_and(|profile| profile.formats.contains(&format)),
            AudioInputSource::DataUrl => self
                .data_url
                .as_ref()
                .is_some_and(|profile| profile.formats.contains(&format)),
            AudioInputSource::Base64 => self
                .base64
                .as_ref()
                .is_some_and(|profile| profile.formats.contains(&format)),
        }
    }

    /// Returns the maximum number of audio parts accepted by one request.
    pub(crate) const fn max_parts(&self) -> u32 {
        self.max_parts
    }

    /// Returns the maximum URL length when the remote source is present.
    pub(crate) fn max_url_length(&self) -> Option<u32> {
        self.remote_url
            .as_ref()
            .map(|profile| profile.max_url_length)
    }

    fn inline_profile(&self, source: AudioInputSource) -> Option<&OwnedInlineAudioInputProfile> {
        match source {
            AudioInputSource::DataUrl => self.data_url.as_ref(),
            AudioInputSource::Base64 => self.base64.as_ref(),
            AudioInputSource::RemoteUrl => None,
        }
    }

    pub(crate) fn max_inline_encoded_bytes(&self, source: AudioInputSource) -> Option<u32> {
        self.inline_profile(source)
            .map(|profile| profile.max_inline_encoded_bytes)
    }

    pub(crate) fn max_inline_decoded_bytes(&self, source: AudioInputSource) -> Option<u32> {
        self.inline_profile(source)
            .map(|profile| profile.max_inline_decoded_bytes)
    }

    pub(crate) fn max_total_inline_encoded_bytes(&self, source: AudioInputSource) -> Option<u32> {
        self.inline_profile(source)
            .map(|profile| profile.max_total_inline_encoded_bytes)
    }

    pub(crate) fn max_total_inline_decoded_bytes(&self, source: AudioInputSource) -> Option<u32> {
        self.inline_profile(source)
            .map(|profile| profile.max_total_inline_decoded_bytes)
    }

    /// Projects source-owned profiles into the stable flat Models v1 representation.
    fn wire_projection(&self) -> AudioInputInterfaceCapabilitiesWire {
        let source_profiles = [
            self.remote_url
                .as_ref()
                .map(|profile| (AudioInputSource::RemoteUrl, profile.formats.as_slice())),
            self.data_url
                .as_ref()
                .map(|profile| (AudioInputSource::DataUrl, profile.formats.as_slice())),
            self.base64
                .as_ref()
                .map(|profile| (AudioInputSource::Base64, profile.formats.as_slice())),
        ];
        let sources = source_profiles
            .iter()
            .flatten()
            .map(|(source, _)| *source)
            .collect::<Vec<_>>();
        let mut format_domains = source_profiles
            .iter()
            .flatten()
            .map(|(_, formats)| *formats);
        let mut formats = format_domains.next().unwrap_or_default().to_vec();
        for domain in format_domains {
            formats.retain(|format| domain.contains(format));
        }

        let inline_profiles = [self.data_url.as_ref(), self.base64.as_ref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let minimum_inline = |value: fn(&OwnedInlineAudioInputProfile) -> u32| {
            inline_profiles.iter().map(|profile| value(profile)).min()
        };
        AudioInputInterfaceCapabilitiesWire {
            sources,
            formats,
            limits: AudioInputLimits {
                max_parts: self.max_parts,
                max_url_length: self
                    .remote_url
                    .as_ref()
                    .map_or(0, |profile| profile.max_url_length),
                max_inline_encoded_bytes: minimum_inline(|profile| {
                    profile.max_inline_encoded_bytes
                })
                .unwrap_or(0),
                max_inline_decoded_bytes: minimum_inline(|profile| {
                    profile.max_inline_decoded_bytes
                })
                .unwrap_or(0),
                max_total_inline_encoded_bytes: minimum_inline(|profile| {
                    profile.max_total_inline_encoded_bytes
                })
                .unwrap_or(0),
                max_total_inline_decoded_bytes: minimum_inline(|profile| {
                    profile.max_total_inline_decoded_bytes
                })
                .unwrap_or(0),
            },
        }
    }
}

impl Serialize for AudioInputInterfaceCapabilities {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.wire_projection().serialize(serializer)
    }
}

/// Public output formats, voices, and response budgets for one TTS-like interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AudioOutputInterfaceCapabilities {
    pub(super) formats: Vec<AudioFormat>,
    pub(super) streaming_formats: Vec<AudioFormat>,
    pub(super) voices: Vec<String>,
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) max_stream_decoded_bytes: u32,
    #[serde(skip)]
    pub(super) json_framing: JsonAudioFraming,
    #[serde(skip)]
    pub(super) sse_framing: SseAudioFraming,
}

impl AudioOutputInterfaceCapabilities {
    /// Converts one complete generated-audio profile into a downstream-safe owned contract.
    fn from_capabilities(value: GeneratedAudioCapabilities, voices: &[&str]) -> Self {
        let json = value.json();
        let sse = value.sse();
        Self {
            formats: json.formats().to_vec(),
            streaming_formats: sse.formats().to_vec(),
            voices: voices.iter().map(|voice| (*voice).to_owned()).collect(),
            max_inline_encoded_bytes: json.max_inline_encoded_bytes(),
            max_inline_decoded_bytes: json.max_inline_decoded_bytes(),
            max_stream_decoded_bytes: sse.max_stream_decoded_bytes(),
            json_framing: json.framing(),
            sse_framing: sse.framing(),
        }
    }

    /// Intersects all candidate profiles without exposing Route or Provider identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let first = values.first()?.to_owned().clone();
        let mut result = first;
        if values.iter().any(|value| {
            value.json_framing != result.json_framing || value.sse_framing != result.sse_framing
        }) {
            return None;
        }
        result
            .formats
            .retain(|format| values.iter().all(|value| value.formats.contains(format)));
        result.streaming_formats.retain(|format| {
            values
                .iter()
                .all(|value| value.streaming_formats.contains(format))
        });
        result
            .voices
            .retain(|voice| values.iter().all(|value| value.voices.contains(voice)));
        result.max_inline_encoded_bytes = values
            .iter()
            .map(|value| value.max_inline_encoded_bytes)
            .min()?;
        result.max_inline_decoded_bytes = values
            .iter()
            .map(|value| value.max_inline_decoded_bytes)
            .min()?;
        result.max_stream_decoded_bytes = values
            .iter()
            .map(|value| value.max_stream_decoded_bytes)
            .min()?;
        (!result.formats.is_empty() && !result.streaming_formats.is_empty()).then_some(result)
    }

    /// Returns whether this interface accepts one output format for the selected streaming mode.
    pub(crate) fn supports_format(&self, format: AudioFormat, streaming: bool) -> bool {
        let formats = if streaming {
            &self.streaming_formats
        } else {
            &self.formats
        };
        formats.contains(&format)
    }

    /// Returns whether this interface accepts one preset voice name.
    pub(crate) fn supports_voice(&self, voice: &str) -> bool {
        self.voices.iter().any(|candidate| candidate == voice)
    }

    /// Returns the non-streaming encoded audio response limit.
    pub(crate) const fn max_inline_encoded_bytes(&self) -> u32 {
        self.max_inline_encoded_bytes
    }

    /// Returns the non-streaming decoded audio response limit.
    pub(crate) const fn max_inline_decoded_bytes(&self) -> u32 {
        self.max_inline_decoded_bytes
    }

    /// Returns the cumulative decoded audio streaming limit.
    pub(crate) const fn max_stream_decoded_bytes(&self) -> u32 {
        self.max_stream_decoded_bytes
    }
}

/// Closed executable audio contract retained by one compiled Public Model interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioInterfaceCapabilities {
    /// General audio content is understood and answered with text.
    AudioUnderstanding {
        /// Accepted business-audio input profile.
        input: AudioInputInterfaceCapabilities,
    },
    /// One audio input is transcribed with an optional supported language selection.
    SpeechRecognition {
        /// Accepted speech input profile.
        input: AudioInputInterfaceCapabilities,
        /// Accepted ASR language selections.
        languages: Vec<AsrLanguage>,
    },
    /// Text is synthesized with one of the advertised preset voices.
    SpeechSynthesis {
        /// Generated-audio delivery and preset-voice profile.
        output: AudioOutputInterfaceCapabilities,
    },
    /// Text is synthesized with a voice described by a user message.
    VoiceDesign {
        /// Generated-audio delivery profile.
        output: AudioOutputInterfaceCapabilities,
    },
    /// Text is synthesized with a voice conditioned by reference audio.
    VoiceClone {
        /// Accepted reference-voice input profile.
        conditioning: AudioInputInterfaceCapabilities,
        /// Generated-audio delivery profile.
        output: AudioOutputInterfaceCapabilities,
    },
}

impl AudioInterfaceCapabilities {
    /// Converts one concrete Target profile into its owned request-time contract.
    fn from_capabilities(value: ExecutableAudioProfile) -> Self {
        match value {
            ExecutableAudioProfile::AudioUnderstanding(profile) => Self::AudioUnderstanding {
                input: AudioInputInterfaceCapabilities::from_capabilities(profile.input()),
            },
            ExecutableAudioProfile::SpeechRecognition(profile) => Self::SpeechRecognition {
                input: AudioInputInterfaceCapabilities::from_capabilities(profile.input()),
                languages: profile.languages().to_vec(),
            },
            ExecutableAudioProfile::SpeechSynthesis(profile) => Self::SpeechSynthesis {
                output: AudioOutputInterfaceCapabilities::from_capabilities(
                    profile.generated_audio(),
                    profile.preset_voices().values(),
                ),
            },
            ExecutableAudioProfile::VoiceDesign(profile) => Self::VoiceDesign {
                output: AudioOutputInterfaceCapabilities::from_capabilities(
                    profile.generated_audio(),
                    &[],
                ),
            },
            ExecutableAudioProfile::VoiceClone(profile) => Self::VoiceClone {
                conditioning: AudioInputInterfaceCapabilities::from_capabilities(
                    profile.voice_conditioning(),
                ),
                output: AudioOutputInterfaceCapabilities::from_capabilities(
                    profile.generated_audio(),
                    &[],
                ),
            },
        }
    }

    /// Intersects same-variant candidate payloads and rejects an empty executable profile.
    fn intersection<'a>(
        values: impl Iterator<Item = Option<&'a Self>>,
    ) -> Result<Option<Self>, ()> {
        // Preserve optional-capability narrowing when any executable candidate omits audio.
        let values = values.collect::<Vec<_>>();
        if values.is_empty() || values.iter().any(|value| value.is_none()) {
            return Ok(None);
        }

        // Fold pairwise intersections so variant mismatch and empty payload sets share one typed boundary.
        let mut values = values.into_iter().flatten();
        let first = values.next().ok_or(())?.clone();
        values
            .try_fold(first, |current, value| current.intersect(value).ok_or(()))
            .map(Some)
    }

    /// Intersects two complete profiles only when their variants and required payloads agree.
    fn intersect(self, other: &Self) -> Option<Self> {
        match (self, other) {
            (
                Self::AudioUnderstanding { input: left },
                Self::AudioUnderstanding { input: right },
            ) => AudioInputInterfaceCapabilities::intersection(
                [Some(&left), Some(right)].into_iter(),
            )
            .map(|input| Self::AudioUnderstanding { input }),
            (
                Self::SpeechRecognition {
                    input: left,
                    mut languages,
                },
                Self::SpeechRecognition {
                    input: right,
                    languages: other_languages,
                },
            ) => {
                let input = AudioInputInterfaceCapabilities::intersection(
                    [Some(&left), Some(right)].into_iter(),
                )?;
                languages.retain(|language| other_languages.contains(language));
                (!languages.is_empty()).then_some(Self::SpeechRecognition { input, languages })
            }
            (Self::SpeechSynthesis { output: left }, Self::SpeechSynthesis { output: right }) => {
                let output = AudioOutputInterfaceCapabilities::intersection(
                    [Some(&left), Some(right)].into_iter(),
                )?;
                (!output.voices.is_empty()).then_some(Self::SpeechSynthesis { output })
            }
            (Self::VoiceDesign { output: left }, Self::VoiceDesign { output: right }) => {
                AudioOutputInterfaceCapabilities::intersection(
                    [Some(&left), Some(right)].into_iter(),
                )
                .map(|output| Self::VoiceDesign { output })
            }
            (
                Self::VoiceClone {
                    conditioning: left_conditioning,
                    output: left_output,
                },
                Self::VoiceClone {
                    conditioning: right_conditioning,
                    output: right_output,
                },
            ) => {
                let conditioning = AudioInputInterfaceCapabilities::intersection(
                    [Some(&left_conditioning), Some(right_conditioning)].into_iter(),
                )?;
                let output = AudioOutputInterfaceCapabilities::intersection(
                    [Some(&left_output), Some(right_output)].into_iter(),
                )?;
                Some(Self::VoiceClone {
                    conditioning,
                    output,
                })
            }
            _ => None,
        }
    }

    /// Returns whether this profile contributes an audio input modality.
    const fn has_input(&self) -> bool {
        matches!(
            self,
            Self::AudioUnderstanding { .. }
                | Self::SpeechRecognition { .. }
                | Self::VoiceClone { .. }
        )
    }

    /// Returns whether this profile contributes an audio output modality.
    const fn has_output(&self) -> bool {
        matches!(
            self,
            Self::SpeechSynthesis { .. } | Self::VoiceDesign { .. } | Self::VoiceClone { .. }
        )
    }

    /// Returns the stable Models extension task projection for this executable profile.
    pub(super) const fn task_projection(&self) -> AudioTaskProjection {
        match self {
            Self::AudioUnderstanding { .. } => AudioTaskProjection::AudioUnderstanding,
            Self::SpeechRecognition { .. } => AudioTaskProjection::Asr,
            Self::SpeechSynthesis { .. } => AudioTaskProjection::Tts,
            Self::VoiceDesign { .. } => AudioTaskProjection::VoiceDesign,
            Self::VoiceClone { .. } => AudioTaskProjection::VoiceClone,
        }
    }

    /// Projects this union into the existing downstream multimodal input object.
    pub(super) fn multimodal_input(
        &self,
    ) -> (
        Option<AudioInputInterfaceCapabilities>,
        Option<AudioInputInterfaceCapabilities>,
    ) {
        match self {
            Self::AudioUnderstanding { input } | Self::SpeechRecognition { input, .. } => {
                (Some(input.clone()), None)
            }
            Self::VoiceClone { conditioning, .. } => (None, Some(conditioning.clone())),
            Self::SpeechSynthesis { .. } | Self::VoiceDesign { .. } => (None, None),
        }
    }

    /// Projects this union into the existing downstream multimodal output object.
    pub(super) fn multimodal_output(&self) -> Option<AudioOutputInterfaceCapabilities> {
        match self {
            Self::SpeechSynthesis { output }
            | Self::VoiceDesign { output }
            | Self::VoiceClone { output, .. } => Some(output.clone()),
            Self::AudioUnderstanding { .. } | Self::SpeechRecognition { .. } => None,
        }
    }
}

/// Existing downstream audio task labels derived from the private executable profile union.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AudioTaskProjection {
    #[serde(rename = "content_understanding")]
    AudioUnderstanding,
    Asr,
    Tts,
    VoiceDesign,
    VoiceClone,
}

impl ImageInputInterfaceCapabilities {
    /// Converts one checked Upstream API image profile into an owned execution contract.
    fn from_capabilities(value: ImageInputCapabilities) -> Self {
        // Copy the static source and detail payloads into the Public Model-owned contract.
        let sources =
            OwnedImageSourceCapabilities::from_capabilities(value.sources(), value.max_parts())
                .expect("checked core image source profile must remain valid");
        let detail = OwnedImageDetailPolicy::from_capabilities(value.detail_policy())
            .expect("checked core image detail policy must remain valid");

        // Revalidate the complete envelope at the ownership boundary.
        Self::new(value.max_parts(), sources, detail)
            .expect("checked core image profile must remain valid")
    }

    /// Intersects every complete Route profile without retaining Provider or Route identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        // Require every executable candidate to contribute a typed image profile.
        let values = values.collect::<Option<Vec<_>>>()?;
        let max_parts = values.iter().map(|value| value.max_parts).min()?;

        // Intersect each source only across candidates that all carry that same source payload.
        let remote = values
            .iter()
            .map(|value| value.sources.remote())
            .collect::<Option<Vec<_>>>()
            .and_then(|values| OwnedRemoteImageInputLimits::intersection(&values));
        let data = values
            .iter()
            .map(|value| value.sources.data())
            .collect::<Option<Vec<_>>>()
            .and_then(|values| OwnedInlineImageInputProfile::intersection(&values, max_parts));
        let sources = match (remote, data) {
            (Some(remote), Some(data)) => {
                OwnedImageSourceCapabilities::RemoteAndInline { remote, data }
            }
            (Some(remote), None) => OwnedImageSourceCapabilities::Remote(remote),
            (None, Some(data)) => OwnedImageSourceCapabilities::Inline(data),
            (None, None) => return None,
        };

        // Apply the full omitted-versus-explicit detail matrix before rebuilding the envelope.
        let detail =
            OwnedImageDetailPolicy::intersection(values.iter().map(|value| &value.detail))?;
        Self::new(max_parts, sources, detail)
    }

    /// Creates a checked owned image envelope.
    fn new(
        max_parts: u32,
        sources: OwnedImageSourceCapabilities,
        detail: OwnedImageDetailPolicy,
    ) -> Option<Self> {
        // Reject an unreachable source payload or invalid detail domain at the final ownership boundary.
        if max_parts == 0 || !sources.is_valid(max_parts) || !detail.is_valid() {
            return None;
        }

        // Store only the closed source and detail unions used by request preflight.
        Some(Self {
            max_parts,
            sources,
            detail,
        })
    }

    /// Projects the owned execution contract into the established flat Models JSON shape.
    fn wire_projection(&self) -> ImageInputInterfaceCapabilitiesWire {
        // Derive source tags and source-specific payloads without persisting flat zero sentinels.
        let sources = match self.sources {
            OwnedImageSourceCapabilities::Remote(_) => vec![ImageInputSource::RemoteUrl],
            OwnedImageSourceCapabilities::Inline(_) => vec![ImageInputSource::DataUrl],
            OwnedImageSourceCapabilities::RemoteAndInline { .. } => {
                vec![ImageInputSource::RemoteUrl, ImageInputSource::DataUrl]
            }
        };
        let remote = self.sources.remote();
        let data = self.sources.data();

        // Flatten absent source payloads to wire-only zero values for the existing DTO schema.
        ImageInputInterfaceCapabilitiesWire {
            sources,
            media_types: data.map_or_else(Vec::new, |profile| profile.media_types.clone()),
            detail: self.detail.wire_projection(),
            limits: ImageInputLimits {
                max_parts: self.max_parts,
                max_url_length: remote.map_or(0, |limits| limits.max_url_length),
                max_inline_encoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_inline_encoded_bytes),
                max_inline_decoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_inline_decoded_bytes),
                max_total_inline_encoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_total_inline_encoded_bytes),
                max_total_inline_decoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_total_inline_decoded_bytes),
            },
        }
    }

    /// Returns whether this interface accepts one source kind.
    pub(crate) fn supports_source(&self, source: ImageInputSource) -> bool {
        match source {
            ImageInputSource::RemoteUrl => self.sources.remote().is_some(),
            ImageInputSource::DataUrl => self.sources.data().is_some(),
            ImageInputSource::FileId => false,
        }
    }

    /// Returns whether this interface accepts one inline media type.
    pub(crate) fn supports_media_type(&self, media_type: ImageMediaType) -> bool {
        self.sources
            .data()
            .is_some_and(|profile| profile.media_types.contains(&media_type))
    }

    /// Returns whether this interface accepts one explicit detail value.
    pub(crate) fn supports_detail(&self, detail: ImageDetail) -> bool {
        self.detail.supports_explicit(detail)
    }

    /// Returns the per-request image part limit.
    pub(crate) const fn max_parts(&self) -> u32 {
        self.max_parts
    }

    /// Returns the remote URL limit only when the source union accepts remote URLs.
    pub(crate) fn max_url_length(&self) -> Option<u32> {
        self.sources.remote().map(|limits| limits.max_url_length)
    }

    /// Returns the per-item encoded limit only when the source union accepts data URLs.
    pub(crate) fn max_inline_encoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_inline_encoded_bytes)
    }

    /// Returns the per-item decoded limit only when the source union accepts data URLs.
    pub(crate) fn max_inline_decoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_inline_decoded_bytes)
    }

    /// Returns the cumulative encoded limit only when the source union accepts data URLs.
    pub(crate) fn max_total_inline_encoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_total_inline_encoded_bytes)
    }

    /// Returns the cumulative decoded limit only when the source union accepts data URLs.
    pub(crate) fn max_total_inline_decoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_total_inline_decoded_bytes)
    }
}

impl OwnedImageSourceCapabilities {
    /// Copies one checked core source union into its owned Public Model representation.
    fn from_capabilities(value: ImageSourceCapabilities, max_parts: u32) -> Option<Self> {
        match value {
            ImageSourceCapabilities::RemoteUrl(remote) => Some(Self::Remote(
                OwnedRemoteImageInputLimits::new(remote.max_url_length())?,
            )),
            ImageSourceCapabilities::DataUrl(data) => Some(Self::Inline(
                OwnedInlineImageInputProfile::from_capabilities(data, max_parts)?,
            )),
            ImageSourceCapabilities::RemoteUrlAndDataUrl { remote, data } => {
                Some(Self::RemoteAndInline {
                    remote: OwnedRemoteImageInputLimits::new(remote.max_url_length())?,
                    data: OwnedInlineImageInputProfile::from_capabilities(data, max_parts)?,
                })
            }
        }
    }

    /// Returns the remote payload when the union accepts remote URLs.
    const fn remote(&self) -> Option<&OwnedRemoteImageInputLimits> {
        match self {
            Self::Remote(remote) | Self::RemoteAndInline { remote, .. } => Some(remote),
            Self::Inline(_) => None,
        }
    }

    /// Returns the inline payload when the union accepts data URLs.
    const fn data(&self) -> Option<&OwnedInlineImageInputProfile> {
        match self {
            Self::Inline(data) | Self::RemoteAndInline { data, .. } => Some(data),
            Self::Remote(_) => None,
        }
    }

    /// Returns whether every retained source payload is complete and reachable.
    fn is_valid(&self, max_parts: u32) -> bool {
        match self {
            Self::Remote(remote) => remote.is_valid(),
            Self::Inline(data) => data.is_valid(max_parts),
            Self::RemoteAndInline { remote, data } => remote.is_valid() && data.is_valid(max_parts),
        }
    }
}

impl OwnedRemoteImageInputLimits {
    /// Creates a positive remote-URL limit.
    const fn new(max_url_length: u32) -> Option<Self> {
        if max_url_length == 0 {
            return None;
        }
        Some(Self { max_url_length })
    }

    /// Intersects complete remote payloads by their conservative URL limit.
    fn intersection(values: &[&Self]) -> Option<Self> {
        Self::new(values.iter().map(|value| value.max_url_length).min()?)
    }

    /// Returns whether this remote payload remains valid.
    const fn is_valid(self) -> bool {
        self.max_url_length > 0
    }
}

impl OwnedInlineImageInputProfile {
    /// Copies one checked static inline profile into owned storage.
    fn from_capabilities(value: InlineImageInputProfile, max_parts: u32) -> Option<Self> {
        let limits = value.limits();
        Self::new(
            value.media_types().to_vec(),
            OwnedInlineImageInputLimits::new(
                limits.max_inline_encoded_bytes(),
                limits.max_inline_decoded_bytes(),
                limits.max_total_inline_encoded_bytes(),
                limits.max_total_inline_decoded_bytes(),
                max_parts,
            )?,
            max_parts,
        )
    }

    /// Intersects inline media types and clamps cross-candidate totals to reachable capacities.
    fn intersection(values: &[&Self], max_parts: u32) -> Option<Self> {
        // Retain only media types supported by every candidate's data-URL payload.
        let mut media_types = values.first()?.media_types.clone();
        media_types.retain(|media_type| {
            values
                .iter()
                .all(|value| value.media_types.contains(media_type))
        });
        if media_types.is_empty() {
            return None;
        }

        // Clamp independently minimized totals and revalidate them with the narrowed part count.
        let limits = OwnedInlineImageInputLimits::intersection(
            &values.iter().map(|value| &value.limits).collect::<Vec<_>>(),
            max_parts,
        )?;
        Self::new(media_types, limits, max_parts)
    }

    /// Creates a checked owned inline profile.
    fn new(
        media_types: Vec<ImageMediaType>,
        limits: OwnedInlineImageInputLimits,
        max_parts: u32,
    ) -> Option<Self> {
        if !is_nonempty_unique(&media_types) || !limits.is_valid(max_parts) {
            return None;
        }
        Some(Self {
            media_types,
            limits,
        })
    }

    /// Returns whether this inline payload remains complete and reachable.
    fn is_valid(&self, max_parts: u32) -> bool {
        is_nonempty_unique(&self.media_types) && self.limits.is_valid(max_parts)
    }
}

impl OwnedInlineImageInputLimits {
    /// Creates positive, internally coherent, and reachable inline limits.
    fn new(
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
        max_parts: u32,
    ) -> Option<Self> {
        // Calculate both reachable totals with checked wide arithmetic.
        let reachable_encoded =
            u64::from(max_inline_encoded_bytes).checked_mul(u64::from(max_parts))?;
        let reachable_decoded =
            u64::from(max_inline_decoded_bytes).checked_mul(u64::from(max_parts))?;

        // Reject zero, one-item-incoherent, or unreachable limit combinations.
        if max_inline_encoded_bytes == 0
            || max_inline_decoded_bytes == 0
            || max_total_inline_encoded_bytes < max_inline_encoded_bytes
            || max_total_inline_decoded_bytes < max_inline_decoded_bytes
            || u64::from(max_total_inline_encoded_bytes) > reachable_encoded
            || u64::from(max_total_inline_decoded_bytes) > reachable_decoded
        {
            return None;
        }
        Some(Self {
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        })
    }

    /// Intersects every numeric dimension and clamps totals to the narrowed reachable capacity.
    fn intersection(values: &[&Self], max_parts: u32) -> Option<Self> {
        // Compute the raw conservative minima for all four inline budget dimensions.
        let max_inline_encoded_bytes = values
            .iter()
            .map(|value| value.max_inline_encoded_bytes)
            .min()?;
        let max_inline_decoded_bytes = values
            .iter()
            .map(|value| value.max_inline_decoded_bytes)
            .min()?;
        let raw_total_encoded = values
            .iter()
            .map(|value| value.max_total_inline_encoded_bytes)
            .min()?;
        let raw_total_decoded = values
            .iter()
            .map(|value| value.max_total_inline_decoded_bytes)
            .min()?;

        // Narrow each total to what the independently minimized per-item and part limits can reach.
        let reachable_encoded =
            u64::from(max_inline_encoded_bytes).checked_mul(u64::from(max_parts))?;
        let reachable_decoded =
            u64::from(max_inline_decoded_bytes).checked_mul(u64::from(max_parts))?;
        let max_total_inline_encoded_bytes =
            u32::try_from(u64::from(raw_total_encoded).min(reachable_encoded)).ok()?;
        let max_total_inline_decoded_bytes =
            u32::try_from(u64::from(raw_total_decoded).min(reachable_decoded)).ok()?;

        // Rebuild through the same checked constructor used by static profile conversion.
        Self::new(
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
            max_parts,
        )
    }

    /// Returns whether all inline limits remain positive, coherent, and reachable.
    fn is_valid(self, max_parts: u32) -> bool {
        Self::new(
            self.max_inline_encoded_bytes,
            self.max_inline_decoded_bytes,
            self.max_total_inline_encoded_bytes,
            self.max_total_inline_decoded_bytes,
            max_parts,
        )
        .is_some()
    }
}

impl OwnedImageDetailPolicy {
    /// Copies one checked core detail policy into its owned Public Model representation.
    fn from_capabilities(value: ImageDetailPolicy) -> Option<Self> {
        match value {
            ImageDetailPolicy::OmittedOnly { default } => Some(Self::OmittedOnly { default }),
            ImageDetailPolicy::Explicit(profile) => {
                Self::explicit(profile.default(), profile.allowed().to_vec())
            }
        }
    }

    /// Intersects omitted behavior and explicit domains according to the complete policy matrix.
    fn intersection<'a>(values: impl Iterator<Item = &'a Self>) -> Option<Self> {
        // Require one common omission default before considering explicit request values.
        let values = values.collect::<Vec<_>>();
        let default = values.first()?.default();
        if values.iter().any(|value| value.default() != default) {
            return None;
        }

        // Any omitted-only candidate narrows the aggregate to omission with the common default.
        if values
            .iter()
            .any(|value| matches!(value, Self::OmittedOnly { .. }))
        {
            return Some(Self::OmittedOnly { default });
        }

        // Intersect every explicit domain; an empty domain safely downgrades to omission-only.
        let mut allowed = values.first()?.allowed().to_vec();
        allowed.retain(|detail| values.iter().all(|value| value.allowed().contains(detail)));
        if allowed.is_empty() {
            Some(Self::OmittedOnly { default })
        } else {
            Self::explicit(default, allowed)
        }
    }

    /// Creates a checked explicit detail policy.
    fn explicit(default: Option<ImageDetail>, allowed: Vec<ImageDetail>) -> Option<Self> {
        is_nonempty_unique(&allowed).then_some(Self::Explicit { default, allowed })
    }

    /// Returns the known effective detail when the request omits the field.
    const fn default(&self) -> Option<ImageDetail> {
        match self {
            Self::OmittedOnly { default } | Self::Explicit { default, .. } => *default,
        }
    }

    /// Returns the explicit detail domain, or an empty slice for omission-only.
    fn allowed(&self) -> &[ImageDetail] {
        match self {
            Self::OmittedOnly { .. } => &[],
            Self::Explicit { allowed, .. } => allowed,
        }
    }

    /// Returns whether the detail policy remains a valid closed state.
    fn is_valid(&self) -> bool {
        match self {
            Self::OmittedOnly { .. } => true,
            Self::Explicit { allowed, .. } => is_nonempty_unique(allowed),
        }
    }

    /// Returns whether one explicit detail value is accepted.
    fn supports_explicit(&self, detail: ImageDetail) -> bool {
        self.allowed().contains(&detail)
    }

    /// Projects the closed detail policy into the existing flat detail object.
    fn wire_projection(&self) -> ImageDetailCapabilities {
        ImageDetailCapabilities {
            default: self.default(),
            allowed: self.allowed().to_vec(),
        }
    }
}

impl Serialize for ImageInputInterfaceCapabilities {
    /// Serializes only the downstream-safe flat projection, never the owned execution union.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.wire_projection().serialize(serializer)
    }
}

/// Normalized file sources exposed by the Models v1 multimodal-input container.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FileInputSource {
    /// Inline Base64 data, with encoding described separately.
    InlineData,
    /// An absolute HTTPS URL fetched by the upstream Provider.
    RemoteUrl,
}

/// Public file budgets guaranteed by every fixed Route for one protocol interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct FileInputLimits {
    pub(super) max_parts: u32,
    pub(super) max_filename_length: u32,
    pub(super) max_url_length: Option<u32>,
    pub(super) max_inline_encoded_bytes: Option<u32>,
    pub(super) max_inline_decoded_bytes: Option<u32>,
    pub(super) max_total_inline_encoded_bytes: Option<u32>,
    pub(super) max_total_inline_decoded_bytes: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct FileDetailCapabilities {
    pub(super) default: FileDetail,
    pub(super) allowed: Vec<FileDetail>,
}

/// Downstream-safe typed file-input contract compiled from executable profiles.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct FileInputInterfaceCapabilities {
    pub(super) sources: Vec<FileInputSource>,
    pub(super) encodings: Vec<FileInlineEncoding>,
    pub(super) media_types: Vec<FileMediaType>,
    detail: Option<FileDetailCapabilities>,
    limits: FileInputLimits,
}

impl FileInputInterfaceCapabilities {
    pub(crate) fn supports_source(&self, source: FileInputSource) -> bool {
        self.sources.contains(&source)
    }

    pub(crate) fn supports_encoding(&self, encoding: FileInlineEncoding) -> bool {
        self.encodings.contains(&encoding)
    }

    pub(crate) fn supports_media_type(&self, media_type: FileMediaType) -> bool {
        self.media_types.contains(&media_type)
    }

    pub(crate) fn supports_detail(&self, detail: FileDetail) -> bool {
        self.detail
            .as_ref()
            .is_some_and(|profile| profile.allowed.contains(&detail))
    }

    pub(crate) const fn max_parts(&self) -> u32 {
        self.limits.max_parts
    }
    pub(crate) const fn max_filename_length(&self) -> u32 {
        self.limits.max_filename_length
    }
    pub(crate) const fn max_url_length(&self) -> Option<u32> {
        self.limits.max_url_length
    }
    pub(crate) const fn max_inline_encoded_bytes(&self) -> Option<u32> {
        self.limits.max_inline_encoded_bytes
    }
    pub(crate) const fn max_inline_decoded_bytes(&self) -> Option<u32> {
        self.limits.max_inline_decoded_bytes
    }
    pub(crate) const fn max_total_inline_encoded_bytes(&self) -> Option<u32> {
        self.limits.max_total_inline_encoded_bytes
    }
    pub(crate) const fn max_total_inline_decoded_bytes(&self) -> Option<u32> {
        self.limits.max_total_inline_decoded_bytes
    }

    fn from_chat(profile: ChatFileInputProfile) -> Self {
        Self::from_parts(
            profile.max_parts(),
            profile.max_filename_length(),
            None,
            Some(profile.inline()),
            None,
        )
    }

    fn from_responses(profile: ResponsesFileInputProfile) -> Self {
        Self::from_parts(
            profile.max_parts(),
            profile.max_filename_length(),
            profile.max_url_length(),
            profile.inline(),
            Some(profile.detail()),
        )
    }

    fn from_parts(
        max_parts: u32,
        max_filename_length: u32,
        max_url_length: Option<u32>,
        inline: Option<crate::core::InlineFileInputProfile>,
        detail: Option<FileDetailProfile>,
    ) -> Self {
        let mut sources = Vec::new();
        let (encodings, media_types, inline_limits) = inline.map_or_else(
            || (Vec::new(), Vec::new(), None),
            |profile| {
                sources.push(FileInputSource::InlineData);
                (
                    profile.encodings().to_vec(),
                    profile.media_types().to_vec(),
                    Some(profile.limits()),
                )
            },
        );
        if max_url_length.is_some() {
            sources.push(FileInputSource::RemoteUrl);
        }
        Self {
            sources,
            encodings,
            media_types,
            detail: detail.map(|profile| FileDetailCapabilities {
                default: profile.default(),
                allowed: profile.allowed().to_vec(),
            }),
            limits: FileInputLimits {
                max_parts,
                max_filename_length,
                max_url_length,
                max_inline_encoded_bytes: inline_limits
                    .map(|value| value.max_inline_encoded_bytes()),
                max_inline_decoded_bytes: inline_limits
                    .map(|value| value.max_inline_decoded_bytes()),
                max_total_inline_encoded_bytes: inline_limits
                    .map(|value| value.max_total_inline_encoded_bytes()),
                max_total_inline_decoded_bytes: inline_limits
                    .map(|value| value.max_total_inline_decoded_bytes()),
            },
        }
    }

    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let first = (*values.first()?).clone();
        let retain_all =
            |candidate: &_| values.iter().all(|value| value.sources.contains(candidate));
        let mut result = first;
        let requires_detail = result.detail.is_some();
        result.sources.retain(retain_all);
        result.encodings.retain(|candidate| {
            values
                .iter()
                .all(|value| value.encodings.contains(candidate))
        });
        result.media_types.retain(|candidate| {
            values
                .iter()
                .all(|value| value.media_types.contains(candidate))
        });
        result.detail = intersect_file_detail(values.iter().map(|value| value.detail.as_ref()));
        result.limits.max_parts = values.iter().map(|value| value.limits.max_parts).min()?;
        result.limits.max_filename_length = values
            .iter()
            .map(|value| value.limits.max_filename_length)
            .min()?;
        result.limits.max_url_length =
            min_optional(values.iter().map(|value| value.limits.max_url_length));
        result.limits.max_inline_encoded_bytes = min_optional(
            values
                .iter()
                .map(|value| value.limits.max_inline_encoded_bytes),
        );
        result.limits.max_inline_decoded_bytes = min_optional(
            values
                .iter()
                .map(|value| value.limits.max_inline_decoded_bytes),
        );
        result.limits.max_total_inline_encoded_bytes = min_optional(
            values
                .iter()
                .map(|value| value.limits.max_total_inline_encoded_bytes),
        );
        result.limits.max_total_inline_decoded_bytes = min_optional(
            values
                .iter()
                .map(|value| value.limits.max_total_inline_decoded_bytes),
        );
        if result.sources.is_empty()
            || result.media_types.is_empty()
            || (result.sources.contains(&FileInputSource::InlineData)
                && result.encodings.is_empty())
            || (requires_detail && result.detail.is_none())
        {
            None
        } else {
            Some(result)
        }
    }
}

fn intersect_file_detail<'a>(
    values: impl Iterator<Item = Option<&'a FileDetailCapabilities>>,
) -> Option<FileDetailCapabilities> {
    let values = values.collect::<Option<Vec<_>>>()?;
    let first = (*values.first()?).clone();
    if values.iter().any(|value| value.default != first.default) {
        return None;
    }
    let mut result = first;
    result
        .allowed
        .retain(|candidate| values.iter().all(|value| value.allowed.contains(candidate)));
    (!result.allowed.is_empty()).then_some(result)
}

fn min_optional(values: impl Iterator<Item = Option<u32>>) -> Option<u32> {
    values.collect::<Option<Vec<_>>>()?.into_iter().min()
}

/// One complete private media contract shared by contribution, aggregation, and request preflight.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct InterfaceMediaCapabilities {
    pub(super) image: Option<ImageInputInterfaceCapabilities>,
    pub(super) audio: Option<AudioInterfaceCapabilities>,
    pub(super) file: Option<FileInputInterfaceCapabilities>,
}

impl InterfaceMediaCapabilities {
    /// Copies one checked Native Chat Target profile into the private interface contract.
    pub(super) fn from_chat(capabilities: ChatCompletionsCapabilities) -> Self {
        Self {
            image: capabilities
                .media
                .image
                .map(ImageInputInterfaceCapabilities::from_capabilities),
            audio: capabilities
                .media
                .audio
                .map(AudioInterfaceCapabilities::from_capabilities),
            file: capabilities
                .media
                .file
                .map(FileInputInterfaceCapabilities::from_chat),
        }
    }

    /// Copies one checked Native Responses Target profile into the private interface contract.
    pub(super) fn from_responses(capabilities: ResponsesCapabilities) -> Self {
        Self {
            image: capabilities
                .media
                .image
                .map(ImageInputInterfaceCapabilities::from_capabilities),
            audio: None,
            file: capabilities
                .media
                .file
                .map(FileInputInterfaceCapabilities::from_responses),
        }
    }

    /// Intersects complete Route media contracts and rejects incompatible audio task variants.
    pub(super) fn intersection<'a>(
        values: impl Iterator<Item = &'a Self> + Clone,
    ) -> Result<Self, ()> {
        Ok(Self {
            image: ImageInputInterfaceCapabilities::intersection(
                values.clone().map(|value| value.image.as_ref()),
            ),
            audio: AudioInterfaceCapabilities::intersection(
                values.clone().map(|value| value.audio.as_ref()),
            )?,
            file: FileInputInterfaceCapabilities::intersection(
                values.map(|value| value.file.as_ref()),
            ),
        })
    }

    pub(super) const fn has_image(&self) -> bool {
        self.image.is_some()
    }

    pub(super) fn has_audio_input(&self) -> bool {
        self.audio
            .as_ref()
            .is_some_and(AudioInterfaceCapabilities::has_input)
    }

    pub(super) fn has_audio_output(&self) -> bool {
        self.audio
            .as_ref()
            .is_some_and(AudioInterfaceCapabilities::has_output)
    }

    pub(super) const fn has_file(&self) -> bool {
        self.file.is_some()
    }

    pub(super) fn clear_image(&mut self) {
        self.image = None;
    }

    pub(super) fn clear_file(&mut self) {
        self.file = None;
    }
}

/// Returns whether a slice is non-empty and duplicate-free without changing its stable order.
fn is_nonempty_unique<T: Eq>(values: &[T]) -> bool {
    !values.is_empty()
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| !values[index + 1..].contains(value))
}

/// Typed multimodal inputs guaranteed by one protocol interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MultimodalInputCapabilities {
    pub(super) image: Option<ImageInputInterfaceCapabilities>,
    pub(super) audio: Option<AudioInputInterfaceCapabilities>,
    pub(super) voice_conditioning: Option<AudioInputInterfaceCapabilities>,
    pub(super) file: Option<FileInputInterfaceCapabilities>,
}

/// Typed multimodal output profiles guaranteed by one protocol interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MultimodalOutputCapabilities {
    pub(super) audio: Option<AudioOutputInterfaceCapabilities>,
}
