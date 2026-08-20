//! Provider-local image ceiling and named Target image profile for ChatGPT.

use crate::core::{
    ImageDetailPolicy, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
    InlineImageInputLimits, InlineImageInputProfile,
};

const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
];

/// Conservative Codex Responses profile for one inline image without explicit detail controls.
pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    1,
    ImageSourceCapabilities::DataUrl(InlineImageInputProfile::new(
        IMAGE_MEDIA_TYPES,
        InlineImageInputLimits::new(
            20 * 1024 * 1024,
            15 * 1024 * 1024,
            20 * 1024 * 1024,
            15 * 1024 * 1024,
        ),
    )),
    ImageDetailPolicy::OmittedOnly { default: None },
);
