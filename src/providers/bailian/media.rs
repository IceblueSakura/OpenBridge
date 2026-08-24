//! Provider-local image ceilings and model-specific Bailian Target profiles.

use crate::core::{
    ImageDetailPolicy, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
    InlineImageInputLimits, InlineImageInputProfile, RemoteImageInputLimits,
};

/// Qwen image surface documented by Alibaba Cloud Model Studio on 2026-08-24.
///
/// The common 250-part ceiling is valid for Base64 and stays below the larger URL-only count of
/// selected Qwen deployments. The inline payload leaves room for the longest 23-byte Data URL
/// prefix inside the documented 20 MB complete-URI ceiling, interpreted conservatively as
/// 20,000,000 bytes. Resolution-dependent MIME narrowing,
/// remote bytes, dimensions, aspect ratio, and download headers remain Provider-enforced.
pub(super) const QWEN_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    250,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: RemoteImageInputLimits::new(8_192),
        data: InlineImageInputProfile::new(
            &[
                ImageMediaType::Bmp,
                ImageMediaType::Jpeg,
                ImageMediaType::Png,
                ImageMediaType::Tiff,
                ImageMediaType::Webp,
                ImageMediaType::Heic,
            ],
            InlineImageInputLimits::new(19_999_976, 14_999_982, 19_999_976, 14_999_982),
        ),
    },
    ImageDetailPolicy::OmittedOnly { default: None },
);

/// Conservative Kimi K3 surface confirmed through the Bailian Chat deployment on 2026-08-24.
pub(super) const KIMI_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    1,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: RemoteImageInputLimits::new(8_192),
        data: InlineImageInputProfile::new(
            &[ImageMediaType::Jpeg, ImageMediaType::Png],
            InlineImageInputLimits::new(1024 * 1024, 768 * 1024, 1024 * 1024, 768 * 1024),
        ),
    },
    ImageDetailPolicy::OmittedOnly { default: None },
);
