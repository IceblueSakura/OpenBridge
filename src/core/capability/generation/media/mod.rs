//! Typed image, audio, and file media capability profiles for generation operations.
//!
//! Source-specific limits live in focused leaves. This facade owns only the complete Chat and
//! Responses media aggregates and preserves the established capability paths.

mod audio;
mod file;
mod image;

pub use audio::{
    AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputSource, AudioUnderstandingProfile,
    ExecutableAudioProfile, GeneratedAudioCapabilities, InlineAudioInputLimits,
    InlineAudioInputProfile, JsonAudioDelivery, JsonAudioFraming, PresetVoiceCapabilities,
    ProviderAudioCeiling, RemoteAudioInputProfile, SpeechRecognitionProfile,
    SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming, VoiceCloneProfile,
    VoiceDesignProfile,
};
pub use file::{
    ChatFileInputProfile, FileDetail, FileDetailProfile, FileInlineEncoding, FileMediaType,
    InlineFileInputLimits, InlineFileInputProfile, ResponsesFileInputProfile,
};
pub(super) use image::image_input_is_subset_of;
pub use image::{
    ImageDetail, ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageInputSource,
    ImageMediaType, ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
    RemoteImageInputLimits,
};

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
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

fn optional_responses_file_input_is_subset_of(
    value: Option<ResponsesFileInputProfile>,
    upper: Option<ResponsesFileInputProfile>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
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

    #[test]
    fn file_profiles_preserve_source_specific_limits_and_subset_ordering() {
        const ENCODINGS: &[FileInlineEncoding] =
            &[FileInlineEncoding::RawBase64, FileInlineEncoding::DataUrl];
        const PDF: &[FileMediaType] = &[FileMediaType::Pdf];
        const DETAILS: &[FileDetail] = &[FileDetail::Auto, FileDetail::Low, FileDetail::High];
        let detail = FileDetailProfile::new(FileDetail::Auto, DETAILS);
        let wide_inline = InlineFileInputProfile::new(
            ENCODINGS,
            PDF,
            InlineFileInputLimits::new(2_048, 1_536, 4_096, 3_072),
        );
        let narrow_inline = InlineFileInputProfile::new(
            &[FileInlineEncoding::DataUrl],
            PDF,
            InlineFileInputLimits::new(1_024, 768, 2_048, 1_536),
        );

        assert!(
            ChatFileInputProfile::new(1, 128, narrow_inline)
                .is_subset_of(ChatFileInputProfile::new(2, 255, wide_inline))
        );
        assert!(
            ResponsesFileInputProfile::new(1, 128, None, Some(narrow_inline), detail).is_subset_of(
                ResponsesFileInputProfile::new(2, 255, Some(8_192), Some(wide_inline), detail)
            )
        );
        assert!(
            !ResponsesFileInputProfile::new(2, 255, Some(8_192), Some(wide_inline), detail)
                .is_subset_of(ResponsesFileInputProfile::new(
                    1,
                    128,
                    None,
                    Some(narrow_inline),
                    detail,
                ))
        );
    }
}
