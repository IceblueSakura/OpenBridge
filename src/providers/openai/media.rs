//! Provider-local image ceiling for OpenAI generation operations.

use crate::core::{
    ImageDetail, ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageMediaType,
    ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
    RemoteImageInputLimits,
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
