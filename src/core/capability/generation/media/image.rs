//! Typed image capability profiles for generation operations.

use serde::Serialize;

/// Standard image source kinds accepted by one protocol-native input profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageInputSource {
    /// An absolute HTTPS URL fetched by the upstream Provider.
    RemoteUrl,
    /// An inline RFC 2397-style Base64 data URL.
    DataUrl,
    /// An opaque Provider-issued file identifier.
    FileId,
}

/// Image media types that OpenBridge can validate without inspecting image content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ImageMediaType {
    /// JPEG image data.
    #[serde(rename = "image/jpeg")]
    Jpeg,
    /// PNG image data.
    #[serde(rename = "image/png")]
    Png,
    /// GIF image data.
    #[serde(rename = "image/gif")]
    Gif,
    /// WebP image data.
    #[serde(rename = "image/webp")]
    Webp,
    /// BMP image data.
    #[serde(rename = "image/bmp")]
    Bmp,
    /// TIFF image data.
    #[serde(rename = "image/tiff")]
    Tiff,
    /// HEIC image data.
    #[serde(rename = "image/heic")]
    Heic,
}

impl ImageMediaType {
    /// Parses one canonical image media type without accepting aliases or parameters.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            "image/bmp" => Some(Self::Bmp),
            "image/tiff" => Some(Self::Tiff),
            "image/heic" => Some(Self::Heic),
            _ => None,
        }
    }
}

/// Standard image-detail values carried by Chat or Responses image parts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    /// Lets the upstream choose the effective image detail.
    Auto,
    /// Requests a low-detail representation.
    Low,
    /// Requests a high-detail representation.
    High,
    /// Requests the original image resolution when supported.
    Original,
}

impl ImageDetail {
    /// Parses one standard image-detail wire value.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "low" => Some(Self::Low),
            "high" => Some(Self::High),
            "original" => Some(Self::Original),
            _ => None,
        }
    }
}

/// Rejects duplicate image-media entries during const profile construction.
const fn assert_unique_image_media_types(media_types: &[ImageMediaType]) {
    let mut left = 0;
    while left < media_types.len() {
        let mut right = left + 1;
        while right < media_types.len() {
            assert!(
                media_types[left] as usize != media_types[right] as usize,
                "image media types must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
    }
}

/// Rejects duplicate explicit image-detail entries during const profile construction.
const fn assert_unique_image_details(details: &[ImageDetail]) {
    let mut left = 0;
    while left < details.len() {
        let mut right = left + 1;
        while right < details.len() {
            assert!(
                details[left] as usize != details[right] as usize,
                "image details must not contain duplicates"
            );
            right += 1;
        }
        left += 1;
    }
}

/// URL-length payload large enough for one absolute HTTPS image reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteImageInputLimits {
    max_url_length: u32,
}

impl RemoteImageInputLimits {
    /// Creates a remote-image limit that can hold at least the shortest absolute HTTPS URL.
    ///
    /// # Panics
    ///
    /// Panics when `max_url_length` is shorter than the nine-byte URL `https://a`.
    pub const fn new(max_url_length: u32) -> Self {
        assert!(
            max_url_length >= 9,
            "remote image URL length limit must allow the nine-byte URL https://a"
        );
        Self { max_url_length }
    }

    /// Returns the maximum UTF-8 byte length of one remote image URL.
    pub const fn max_url_length(self) -> u32 {
        self.max_url_length
    }

    /// Returns whether this remote payload stays within another payload ceiling.
    const fn is_subset_of(self, upper: Self) -> bool {
        self.max_url_length <= upper.max_url_length
    }
}

/// Wire-reachable per-item and cumulative budgets for inline image data URLs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineImageInputLimits {
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

impl InlineImageInputLimits {
    /// Creates coherent inline-image budgets independent of request cardinality.
    ///
    /// [`ImageInputCapabilities::new`] additionally verifies that each cumulative budget is
    /// reachable under the enclosing positive `max_parts` limit.
    ///
    /// # Panics
    ///
    /// Panics when the encoded budget cannot hold one four-byte Base64 quantum, the decoded budget
    /// cannot hold one byte, or a cumulative limit cannot cover one image.
    pub const fn new(
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
    ) -> Self {
        // Validate wire-reachable per-item budgets before relating cumulative capacity.
        assert!(
            max_inline_encoded_bytes >= 4,
            "inline image encoded-byte limit must allow one four-byte Base64 quantum"
        );
        assert!(
            max_inline_decoded_bytes >= 1,
            "inline image decoded-byte limit must allow one byte"
        );
        assert!(
            max_total_inline_encoded_bytes >= max_inline_encoded_bytes,
            "total encoded-byte limit must cover one inline image"
        );
        assert!(
            max_total_inline_decoded_bytes >= max_inline_decoded_bytes,
            "total decoded-byte limit must cover one inline image"
        );

        // Construct the source-local limits after their intrinsic invariants hold.
        Self {
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        }
    }

    /// Returns the maximum Base64 payload length of one inline image.
    pub const fn max_inline_encoded_bytes(self) -> u32 {
        self.max_inline_encoded_bytes
    }

    /// Returns the maximum decoded length of one inline image.
    pub const fn max_inline_decoded_bytes(self) -> u32 {
        self.max_inline_decoded_bytes
    }

    /// Returns the cumulative Base64 payload limit across inline images.
    pub const fn max_total_inline_encoded_bytes(self) -> u32 {
        self.max_total_inline_encoded_bytes
    }

    /// Returns the cumulative decoded-byte limit across inline images.
    pub const fn max_total_inline_decoded_bytes(self) -> u32 {
        self.max_total_inline_decoded_bytes
    }

    /// Verifies that cumulative limits are reachable under the enclosing part count.
    const fn assert_reachable(self, max_parts: u32) {
        assert!(
            self.max_total_inline_encoded_bytes as u64
                <= self.max_inline_encoded_bytes as u64 * max_parts as u64,
            "total encoded-byte limit exceeds the image per-part capacity"
        );
        assert!(
            self.max_total_inline_decoded_bytes as u64
                <= self.max_inline_decoded_bytes as u64 * max_parts as u64,
            "total decoded-byte limit exceeds the image per-part capacity"
        );
    }

    /// Returns whether this inline budget stays within another budget ceiling.
    const fn is_subset_of(self, upper: Self) -> bool {
        self.max_inline_encoded_bytes <= upper.max_inline_encoded_bytes
            && self.max_inline_decoded_bytes <= upper.max_inline_decoded_bytes
            && self.max_total_inline_encoded_bytes <= upper.max_total_inline_encoded_bytes
            && self.max_total_inline_decoded_bytes <= upper.max_total_inline_decoded_bytes
    }
}

/// Non-empty media-type set and complete budgets for inline image data URLs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineImageInputProfile {
    media_types: &'static [ImageMediaType],
    limits: InlineImageInputLimits,
}

impl InlineImageInputProfile {
    /// Creates a checked inline-image profile.
    ///
    /// # Panics
    ///
    /// Panics when `media_types` is empty or contains duplicates.
    pub const fn new(
        media_types: &'static [ImageMediaType],
        limits: InlineImageInputLimits,
    ) -> Self {
        // Validate the set-valued media domain before binding its checked limits.
        assert!(
            !media_types.is_empty(),
            "inline image media types must not be empty"
        );
        assert_unique_image_media_types(media_types);

        // Construct the complete inline source payload.
        Self {
            media_types,
            limits,
        }
    }

    /// Returns the accepted inline image media types.
    pub const fn media_types(self) -> &'static [ImageMediaType] {
        self.media_types
    }

    /// Returns the complete inline-image budgets.
    pub const fn limits(self) -> InlineImageInputLimits {
        self.limits
    }

    /// Returns whether this inline payload stays within another payload ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.media_types
            .iter()
            .all(|media_type| upper.media_types.contains(media_type))
            && self.limits.is_subset_of(upper.limits)
    }
}

/// Closed source-payload union shared by Provider ceilings and executable image profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageSourceCapabilities {
    /// Accepts remote HTTPS URLs with the supplied URL budget.
    RemoteUrl(RemoteImageInputLimits),
    /// Accepts inline data URLs with the supplied media and byte budgets.
    DataUrl(InlineImageInputProfile),
    /// Accepts both implemented source kinds with independently owned payloads.
    RemoteUrlAndDataUrl {
        /// Complete remote-URL payload.
        remote: RemoteImageInputLimits,
        /// Complete inline data-URL payload.
        data: InlineImageInputProfile,
    },
}

impl ImageSourceCapabilities {
    /// Returns the remote-URL payload when this source union accepts remote images.
    pub const fn remote(self) -> Option<RemoteImageInputLimits> {
        match self {
            Self::RemoteUrl(remote) | Self::RemoteUrlAndDataUrl { remote, .. } => Some(remote),
            Self::DataUrl(_) => None,
        }
    }

    /// Returns the inline data-URL payload when this source union accepts inline images.
    pub const fn data(self) -> Option<InlineImageInputProfile> {
        match self {
            Self::DataUrl(data) | Self::RemoteUrlAndDataUrl { data, .. } => Some(data),
            Self::RemoteUrl(_) => None,
        }
    }

    /// Verifies every inline payload against the enclosing positive part count.
    const fn assert_reachable(self, max_parts: u32) {
        if let Some(data) = self.data() {
            data.limits.assert_reachable(max_parts);
        }
    }

    /// Returns whether each retained source payload stays within the same source in the ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        // Compare the remote payload only when the narrower profile retains that source.
        let remote_is_subset = match self.remote() {
            Some(remote) => upper
                .remote()
                .is_some_and(|upper| remote.is_subset_of(upper)),
            None => true,
        };

        // Compare the inline payload independently so one source cannot satisfy another.
        let data_is_subset = match self.data() {
            Some(data) => upper.data().is_some_and(|upper| data.is_subset_of(upper)),
            None => true,
        };
        remote_is_subset && data_is_subset
    }
}

/// Explicit image-detail domain plus the known behavior when `detail` is omitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageDetailProfile {
    default: Option<ImageDetail>,
    allowed: &'static [ImageDetail],
}

impl ImageDetailProfile {
    /// Creates a checked explicit image-detail profile.
    ///
    /// The omitted default is independent of the explicit domain and need not be a member.
    ///
    /// # Panics
    ///
    /// Panics when `allowed` is empty or contains duplicates.
    pub const fn new(default: Option<ImageDetail>, allowed: &'static [ImageDetail]) -> Self {
        assert!(
            !allowed.is_empty(),
            "explicit image details must not be empty"
        );
        assert_unique_image_details(allowed);
        Self { default, allowed }
    }

    /// Returns the known effective detail when the wire field is omitted.
    pub const fn default(self) -> Option<ImageDetail> {
        self.default
    }

    /// Returns the non-empty explicit image-detail domain.
    pub const fn allowed(self) -> &'static [ImageDetail] {
        self.allowed
    }

    /// Returns whether this explicit profile stays within another profile ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.default == upper.default
            && self
                .allowed
                .iter()
                .all(|detail| upper.allowed.contains(detail))
    }
}

/// Closed omitted-only or explicit image-detail request policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageDetailPolicy {
    /// Only omission is accepted, with an optional known effective default.
    OmittedOnly {
        /// Known effective detail when the request omits the wire field.
        default: Option<ImageDetail>,
    },
    /// Explicit values are accepted according to the checked profile.
    Explicit(ImageDetailProfile),
}

impl ImageDetailPolicy {
    /// Returns the known effective detail when the wire field is omitted.
    pub const fn default(self) -> Option<ImageDetail> {
        match self {
            Self::OmittedOnly { default } => default,
            Self::Explicit(profile) => profile.default,
        }
    }

    /// Returns the explicit profile, or `None` for an omitted-only policy.
    pub const fn explicit(self) -> Option<ImageDetailProfile> {
        match self {
            Self::OmittedOnly { .. } => None,
            Self::Explicit(profile) => Some(profile),
        }
    }

    /// Returns whether this detail policy stays within another policy ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        // Preserve the exact known behavior of an omitted wire field.
        if self.default() != upper.default() {
            return false;
        }

        // Apply the closed omitted-only versus explicit subset matrix.
        match (self, upper) {
            (Self::OmittedOnly { .. }, _) => true,
            (Self::Explicit(value), Self::Explicit(upper)) => value.is_subset_of(upper),
            (Self::Explicit(_), Self::OmittedOnly { .. }) => false,
        }
    }
}

/// Provider or Upstream API ceiling for protocol-native image inputs.
///
/// Every accepted source owns its complete payload. Byte limits apply to the Base64 payload after
/// the data-URL prefix. The gateway request-body limit remains an independent deployment-wide
/// ceiling and may be smaller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageInputCapabilities {
    max_parts: u32,
    sources: ImageSourceCapabilities,
    detail_policy: ImageDetailPolicy,
}

impl ImageInputCapabilities {
    /// Creates a complete image-input envelope and validates cross-field reachability.
    ///
    /// # Panics
    ///
    /// Panics when `max_parts` is zero or an inline cumulative budget is unreachable.
    pub const fn new(
        max_parts: u32,
        sources: ImageSourceCapabilities,
        detail_policy: ImageDetailPolicy,
    ) -> Self {
        // Validate outer cardinality before checking source-specific aggregate reachability.
        assert!(max_parts > 0, "image input max_parts must be positive");
        sources.assert_reachable(max_parts);

        // Construct the closed envelope after every primitive and cross-field invariant holds.
        Self {
            max_parts,
            sources,
            detail_policy,
        }
    }

    /// Returns the maximum number of image parts in one request.
    pub const fn max_parts(self) -> u32 {
        self.max_parts
    }

    /// Returns the closed source-payload union.
    pub const fn sources(self) -> ImageSourceCapabilities {
        self.sources
    }

    /// Returns the omitted-only or explicit image-detail policy.
    pub const fn detail_policy(self) -> ImageDetailPolicy {
        self.detail_policy
    }

    /// Returns whether this profile stays within another Provider or API ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.max_parts <= upper.max_parts
            && self.sources.is_subset_of(upper.sources)
            && self.detail_policy.is_subset_of(upper.detail_policy)
    }
}

/// Returns whether one optional image profile is conservatively bounded by another.
pub(in crate::core::capability::generation) fn image_input_is_subset_of(
    value: Option<ImageInputCapabilities>,
    upper: Option<ImageInputCapabilities>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
        (Some(_), None) => false,
    }
}
