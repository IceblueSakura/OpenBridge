//! Provider-local image ceiling and direct DeepSeek Vision Target profile.

use crate::core::{
    ImageDetail, ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageMediaType,
    ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
    RemoteImageInputLimits,
};

/// Image surface documented by DeepSeek and exercised on the direct Vision deployment.
///
/// On 2026-08-24, both Native protocols accepted remote HTTPS images, inline JPEG/PNG/GIF/WebP,
/// all four explicit detail values, and two-image requests. The inline profile maps the official
/// 32 MiB decoded single-image ceiling to its exact Base64 bound and applies that bound cumulatively,
/// leaving room inside the Provider's 48 MiB JSON body ceiling. Remote byte and cross-source totals
/// remain Provider-enforced because the current profile cannot express them.
pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    600,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: RemoteImageInputLimits::new(8_192),
        data: InlineImageInputProfile::new(
            &[
                ImageMediaType::Jpeg,
                ImageMediaType::Png,
                ImageMediaType::Gif,
                ImageMediaType::Webp,
            ],
            InlineImageInputLimits::new(44_739_244, 32 * 1024 * 1024, 44_739_244, 32 * 1024 * 1024),
        ),
    },
    ImageDetailPolicy::Explicit(ImageDetailProfile::new(
        None,
        &[
            ImageDetail::Auto,
            ImageDetail::Low,
            ImageDetail::High,
            ImageDetail::Original,
        ],
    )),
);
