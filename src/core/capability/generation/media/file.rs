//! Typed file capability profiles for generation operations.

use serde::Serialize;

/// Inline encoding accepted by a protocol-native file input profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileInlineEncoding {
    /// A pure Base64 string whose media type is inferred from the validated filename.
    RawBase64,
    /// An RFC 2397-style Base64 data URL carrying an explicit media type.
    DataUrl,
}

/// File media categories OpenBridge can validate without parsing file content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FileMediaType {
    /// Portable Document Format.
    #[serde(rename = "application/pdf")]
    Pdf,
}

impl FileMediaType {
    /// Parses one canonical file media type without aliases or parameters.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "application/pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// Infers the closed media category from one validated filename suffix.
    pub(crate) fn from_filename(value: &str) -> Option<Self> {
        value
            .rsplit_once('.')
            .and_then(|(_, suffix)| suffix.eq_ignore_ascii_case("pdf").then_some(Self::Pdf))
    }
}

/// PDF page-image detail accepted by Responses `input_file`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileDetail {
    /// Provider-selected PDF detail.
    Auto,
    /// Lower-token PDF page images.
    Low,
    /// Higher-detail PDF page images.
    High,
}

impl FileDetail {
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "low" => Some(Self::Low),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

/// Non-empty Responses PDF detail domain with one allowed default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileDetailProfile {
    default: FileDetail,
    allowed: &'static [FileDetail],
}

impl FileDetailProfile {
    pub const fn new(default: FileDetail, allowed: &'static [FileDetail]) -> Self {
        assert!(!allowed.is_empty(), "file detail domain must not be empty");
        assert_unique_file_details(allowed);
        assert!(
            file_details_contain(allowed, default),
            "file detail default must be allowed"
        );
        Self { default, allowed }
    }

    pub const fn default(self) -> FileDetail {
        self.default
    }

    pub const fn allowed(self) -> &'static [FileDetail] {
        self.allowed
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.default == upper.default
            && self
                .allowed
                .iter()
                .all(|value| upper.allowed.contains(value))
    }
}

/// Encoded and safely decoded byte limits for inline file input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineFileInputLimits {
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

impl InlineFileInputLimits {
    /// Creates positive per-item and cumulative limits whose cumulative values cover one item.
    pub const fn new(
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
    ) -> Self {
        assert!(
            max_inline_encoded_bytes > 0,
            "inline file encoded limit must be positive"
        );
        assert!(
            max_inline_decoded_bytes > 0,
            "inline file decoded limit must be positive"
        );
        assert!(max_total_inline_encoded_bytes >= max_inline_encoded_bytes);
        assert!(max_total_inline_decoded_bytes >= max_inline_decoded_bytes);
        Self {
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        }
    }

    /// Returns the per-item encoded byte ceiling.
    pub const fn max_inline_encoded_bytes(self) -> u32 {
        self.max_inline_encoded_bytes
    }
    /// Returns the per-item decoded byte ceiling.
    pub const fn max_inline_decoded_bytes(self) -> u32 {
        self.max_inline_decoded_bytes
    }
    /// Returns the cumulative encoded byte ceiling.
    pub const fn max_total_inline_encoded_bytes(self) -> u32 {
        self.max_total_inline_encoded_bytes
    }
    /// Returns the cumulative decoded byte ceiling.
    pub const fn max_total_inline_decoded_bytes(self) -> u32 {
        self.max_total_inline_decoded_bytes
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.max_inline_encoded_bytes <= upper.max_inline_encoded_bytes
            && self.max_inline_decoded_bytes <= upper.max_inline_decoded_bytes
            && self.max_total_inline_encoded_bytes <= upper.max_total_inline_encoded_bytes
            && self.max_total_inline_decoded_bytes <= upper.max_total_inline_decoded_bytes
    }
}

/// Non-empty inline file source contract shared by the two native Generation protocols.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineFileInputProfile {
    encodings: &'static [FileInlineEncoding],
    media_types: &'static [FileMediaType],
    limits: InlineFileInputLimits,
}

impl InlineFileInputProfile {
    /// Creates a duplicate-free inline contract with at least one encoding and media type.
    pub const fn new(
        encodings: &'static [FileInlineEncoding],
        media_types: &'static [FileMediaType],
        limits: InlineFileInputLimits,
    ) -> Self {
        assert!(
            !encodings.is_empty(),
            "inline file encodings must not be empty"
        );
        assert!(
            !media_types.is_empty(),
            "inline file media types must not be empty"
        );
        assert_unique_file_encodings(encodings);
        assert_unique_file_media_types(media_types);
        Self {
            encodings,
            media_types,
            limits,
        }
    }

    /// Returns accepted inline encodings.
    pub const fn encodings(self) -> &'static [FileInlineEncoding] {
        self.encodings
    }
    /// Returns accepted file media categories.
    pub const fn media_types(self) -> &'static [FileMediaType] {
        self.media_types
    }
    /// Returns inline byte ceilings.
    pub const fn limits(self) -> InlineFileInputLimits {
        self.limits
    }

    fn is_subset_of(self, upper: Self) -> bool {
        self.encodings
            .iter()
            .all(|value| upper.encodings.contains(value))
            && self
                .media_types
                .iter()
                .all(|value| upper.media_types.contains(value))
            && self.limits.is_subset_of(upper.limits)
    }
}

/// Executable Chat Completions `file` content-part contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatFileInputProfile {
    max_parts: u32,
    max_filename_length: u32,
    inline: InlineFileInputProfile,
}

impl ChatFileInputProfile {
    /// Creates a Chat file contract; hosted IDs remain intentionally unrepresentable.
    pub const fn new(
        max_parts: u32,
        max_filename_length: u32,
        inline: InlineFileInputProfile,
    ) -> Self {
        assert!(max_parts > 0, "Chat file part limit must be positive");
        assert!(
            max_filename_length > 0,
            "Chat filename limit must be positive"
        );
        Self {
            max_parts,
            max_filename_length,
            inline,
        }
    }

    /// Returns the maximum file part count.
    pub const fn max_parts(self) -> u32 {
        self.max_parts
    }
    /// Returns the maximum filename byte length.
    pub const fn max_filename_length(self) -> u32 {
        self.max_filename_length
    }
    /// Returns the required inline source contract.
    pub const fn inline(self) -> InlineFileInputProfile {
        self.inline
    }

    pub(super) fn is_subset_of(self, upper: Self) -> bool {
        self.max_parts <= upper.max_parts
            && self.max_filename_length <= upper.max_filename_length
            && self.inline.is_subset_of(upper.inline)
    }
}

/// Executable Responses `input_file` contract with optional inline and remote HTTPS sources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResponsesFileInputProfile {
    max_parts: u32,
    max_filename_length: u32,
    max_url_length: Option<u32>,
    inline: Option<InlineFileInputProfile>,
    detail: FileDetailProfile,
}

impl ResponsesFileInputProfile {
    /// Creates a Responses file contract with at least one executable non-resource source.
    pub const fn new(
        max_parts: u32,
        max_filename_length: u32,
        max_url_length: Option<u32>,
        inline: Option<InlineFileInputProfile>,
        detail: FileDetailProfile,
    ) -> Self {
        assert!(max_parts > 0, "Responses file part limit must be positive");
        assert!(
            max_filename_length > 0,
            "Responses filename limit must be positive"
        );
        assert!(
            max_url_length.is_some() || inline.is_some(),
            "Responses file profile needs one source"
        );
        if let Some(value) = max_url_length {
            assert!(value > 0, "file URL limit must be positive");
        }
        Self {
            max_parts,
            max_filename_length,
            max_url_length,
            inline,
            detail,
        }
    }

    /// Returns the maximum file part count.
    pub const fn max_parts(self) -> u32 {
        self.max_parts
    }
    /// Returns the maximum filename byte length.
    pub const fn max_filename_length(self) -> u32 {
        self.max_filename_length
    }
    /// Returns the remote URL byte limit when that source is enabled.
    pub const fn max_url_length(self) -> Option<u32> {
        self.max_url_length
    }
    /// Returns the inline source contract when enabled.
    pub const fn inline(self) -> Option<InlineFileInputProfile> {
        self.inline
    }

    /// Returns the PDF detail domain.
    pub const fn detail(self) -> FileDetailProfile {
        self.detail
    }

    pub(super) fn is_subset_of(self, upper: Self) -> bool {
        self.max_parts <= upper.max_parts
            && self.max_filename_length <= upper.max_filename_length
            && optional_limit_is_subset_of(self.max_url_length, upper.max_url_length)
            && optional_inline_file_is_subset_of(self.inline, upper.inline)
            && self.detail.is_subset_of(upper.detail)
    }
}

const fn assert_unique_file_encodings(values: &[FileInlineEncoding]) {
    let mut left = 0;
    while left < values.len() {
        let mut right = left + 1;
        while right < values.len() {
            assert!(
                values[left] as u8 != values[right] as u8,
                "file encodings must be unique"
            );
            right += 1;
        }
        left += 1;
    }
}

const fn assert_unique_file_media_types(values: &[FileMediaType]) {
    let mut left = 0;
    while left < values.len() {
        let mut right = left + 1;
        while right < values.len() {
            assert!(
                values[left] as u8 != values[right] as u8,
                "file media types must be unique"
            );
            right += 1;
        }
        left += 1;
    }
}

const fn assert_unique_file_details(values: &[FileDetail]) {
    let mut left = 0;
    while left < values.len() {
        let mut right = left + 1;
        while right < values.len() {
            assert!(
                values[left] as u8 != values[right] as u8,
                "file details must be unique"
            );
            right += 1;
        }
        left += 1;
    }
}

const fn file_details_contain(values: &[FileDetail], expected: FileDetail) -> bool {
    let mut index = 0;
    while index < values.len() {
        if values[index] as u8 == expected as u8 {
            return true;
        }
        index += 1;
    }
    false
}

fn optional_inline_file_is_subset_of(
    value: Option<InlineFileInputProfile>,
    upper: Option<InlineFileInputProfile>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

const fn optional_limit_is_subset_of(value: Option<u32>, upper: Option<u32>) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value <= upper,
    }
}
