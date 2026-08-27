//! Provider-local image ceiling and named Target image profile for Zhipu China.

use crate::core::{
    ImageDetailPolicy, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
    InlineImageInputLimits, InlineImageInputProfile, RemoteImageInputLimits,
};

/// Bounded image surface for the Zhipu China GLM-5.3-Flash Chat Target.
///
/// The official model page declares multiple URL or Base64 images:
/// <https://docs.bigmodel.cn/cn/guide/models/vlm/glm-5.3-flash>. One PNG data URL was exercised
/// successfully on 2026-08-27. OpenBridge keeps the executable envelope to JPEG/PNG, four parts, and
/// bounded URL/decoded sizes instead of exposing the Provider's wider unbounded media statement.
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
