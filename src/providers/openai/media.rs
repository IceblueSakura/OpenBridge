//! Provider-local image ceiling for OpenAI generation operations.

use crate::core::{
    ChatFileInputProfile, FileDetail, FileDetailProfile, FileInlineEncoding, FileMediaType,
    ImageDetail, ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageMediaType,
    ImageSourceCapabilities, InlineFileInputLimits, InlineFileInputProfile, InlineImageInputLimits,
    InlineImageInputProfile, RemoteImageInputLimits, ResponsesFileInputProfile,
};

const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
];
const IMAGE_DETAILS: &[ImageDetail] = &[
    ImageDetail::Auto,
    ImageDetail::Low,
    ImageDetail::High,
    ImageDetail::Original,
];
const IMAGE_REMOTE_LIMITS: RemoteImageInputLimits = RemoteImageInputLimits::new(8_192);
const IMAGE_INLINE_LIMITS: InlineImageInputLimits = InlineImageInputLimits::new(
    20 * 1024 * 1024,
    15 * 1024 * 1024,
    50 * 1024 * 1024,
    38 * 1024 * 1024,
);
const IMAGE_INLINE_PROFILE: InlineImageInputProfile =
    InlineImageInputProfile::new(IMAGE_MEDIA_TYPES, IMAGE_INLINE_LIMITS);
const IMAGE_DETAIL_PROFILE: ImageDetailProfile =
    ImageDetailProfile::new(Some(ImageDetail::Auto), IMAGE_DETAILS);

pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    500,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: IMAGE_REMOTE_LIMITS,
        data: IMAGE_INLINE_PROFILE,
    },
    ImageDetailPolicy::Explicit(IMAGE_DETAIL_PROFILE),
);

const FILE_ENCODINGS: &[FileInlineEncoding] =
    &[FileInlineEncoding::RawBase64, FileInlineEncoding::DataUrl];
const FILE_MEDIA_TYPES: &[FileMediaType] = &[FileMediaType::Pdf];
const FILE_DETAILS: &[FileDetail] = &[FileDetail::Auto, FileDetail::Low, FileDetail::High];
const FILE_DETAIL_PROFILE: FileDetailProfile =
    FileDetailProfile::new(FileDetail::Auto, FILE_DETAILS);
const FILE_INLINE_LIMITS: InlineFileInputLimits =
    InlineFileInputLimits::new(69_905_068, 52_428_800, 69_905_068, 52_428_800);
const FILE_INLINE_INPUT: InlineFileInputProfile =
    InlineFileInputProfile::new(FILE_ENCODINGS, FILE_MEDIA_TYPES, FILE_INLINE_LIMITS);

/// OpenAI Chat file-input wire ceiling; concrete model Targets remain deny-by-default.
pub(super) const CHAT_FILE_INPUT: ChatFileInputProfile =
    ChatFileInputProfile::new(10, 255, FILE_INLINE_INPUT);
/// OpenAI Responses file-input wire ceiling; concrete model Targets remain deny-by-default.
pub(super) const RESPONSES_FILE_INPUT: ResponsesFileInputProfile = ResponsesFileInputProfile::new(
    10,
    255,
    Some(8_192),
    Some(FILE_INLINE_INPUT),
    FILE_DETAIL_PROFILE,
);
