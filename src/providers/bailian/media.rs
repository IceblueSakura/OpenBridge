//! Provider-local image ceiling and probed Bailian Target profile.

use crate::core::{
    ImageDetailPolicy, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
    InlineImageInputLimits, InlineImageInputProfile, RemoteImageInputLimits,
};

/// Single-image surface proven across every registered image-capable Bailian Target.
///
/// On 2026-08-24, remote JPEG, inline JPEG, and 16x16 inline PNG succeeded through every
/// registered native protocol for Qwen3.7 Plus, Qwen3.8 Max, Qwen3.8 27B, and Kimi K3. On both
/// Qwen3.8 models, a 1x1 PNG reached the Provider but was rejected because each side must exceed 10
/// pixels; the current media type system cannot express that observed pixel-dimension floor.
pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
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
