//! Provider-local image ceiling and named Target image profile for OpenRouter.

use crate::core::{
    ImageDetailPolicy, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
    InlineImageInputLimits, InlineImageInputProfile, RemoteImageInputLimits,
};

/// Image surface confirmed for selected OpenRouter Chat and Responses targets.
///
/// PNG/JPEG data URLs and remote JPEGs are proven on Gemini 3.7 Flash and Grok 4.6 native Chat and
/// Responses paths (2026-08-24); no other media type was exercised for those targets.
pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    4,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: RemoteImageInputLimits::new(8_192),
        data: InlineImageInputProfile::new(
            &[ImageMediaType::Jpeg, ImageMediaType::Png],
            InlineImageInputLimits::new(
                20 * 1024 * 1024,
                15 * 1024 * 1024,
                20 * 1024 * 1024,
                15 * 1024 * 1024,
            ),
        ),
    },
    ImageDetailPolicy::OmittedOnly { default: None },
);
