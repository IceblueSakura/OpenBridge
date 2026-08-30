//! Portable media resources and their origin-aware sources.

use super::{BoundedBytes, MediaType, OpaqueState, SemanticValidationError, UrlValue};

/// Origin-bound Provider resource reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReference(OpaqueState);

impl ProviderReference {
    /// Creates a reference only when the opaque state carries a Provider origin.
    pub fn new(state: OpaqueState) -> Result<Self, SemanticValidationError> {
        if state.origin().is_none() {
            return Err(SemanticValidationError::MissingProviderOrigin);
        }
        if state.kind() != super::OpaqueKind::Extension {
            return Err(SemanticValidationError::InvalidProviderReferenceKind);
        }
        Ok(Self(state))
    }

    /// Returns the origin-bound opaque reference.
    pub const fn state(&self) -> &OpaqueState {
        &self.0
    }
}

/// Non-empty bounded inline resource payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineResource(BoundedBytes);

impl InlineResource {
    /// Creates a non-empty inline resource from already bounded bytes.
    pub fn new(bytes: BoundedBytes) -> Result<Self, SemanticValidationError> {
        if bytes.is_empty() {
            return Err(SemanticValidationError::EmptyInlineResource);
        }
        Ok(Self(bytes))
    }

    /// Returns the bounded inline bytes.
    pub const fn bytes(&self) -> &BoundedBytes {
        &self.0
    }
}

/// A Provider-independent resource source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceSource {
    /// A remote or protocol-provided absolute URL.
    Url(UrlValue),
    /// Inline resource bytes.
    Inline(InlineResource),
    /// A Provider-owned reference that must remain opaque and origin-bound.
    ProviderReference(ProviderReference),
}

impl ResourceSource {
    /// Returns the URL source, if this is a URL resource.
    pub fn as_url(&self) -> Option<&UrlValue> {
        match self {
            Self::Url(value) => Some(value),
            Self::Inline(_) | Self::ProviderReference(_) => None,
        }
    }

    /// Returns the inline source, if this is an inline resource.
    pub fn as_inline(&self) -> Option<&InlineResource> {
        match self {
            Self::Inline(value) => Some(value),
            Self::Url(_) | Self::ProviderReference(_) => None,
        }
    }

    /// Returns the Provider reference, if this is an opaque reference source.
    pub fn as_provider_reference(&self) -> Option<&ProviderReference> {
        match self {
            Self::ProviderReference(value) => Some(value),
            Self::Url(_) | Self::Inline(_) => None,
        }
    }
}

/// Requested image-quality/detail hint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ImageDetail {
    /// Let the Target choose its default detail.
    Auto,
    /// Prefer lower detail/cost.
    Low,
    /// Prefer higher detail.
    High,
}

/// Image resource with optional declared media type and detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageResource {
    source: ResourceSource,
    media_type: Option<MediaType>,
    detail: Option<ImageDetail>,
}

impl ImageResource {
    /// Creates an image resource from a source and optional MIME type.
    pub const fn new(
        source: ResourceSource,
        media_type: Option<MediaType>,
        detail: Option<ImageDetail>,
    ) -> Self {
        Self {
            source,
            media_type,
            detail,
        }
    }

    /// Returns the resource source.
    pub fn source(&self) -> &ResourceSource {
        &self.source
    }

    /// Returns the declared media type, when present.
    pub fn media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }

    /// Returns the requested image detail.
    pub const fn detail(&self) -> Option<ImageDetail> {
        self.detail
    }
}

/// Audio resource with optional declared media type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioResource {
    source: ResourceSource,
    media_type: Option<MediaType>,
}

impl AudioResource {
    /// Creates an audio resource from a source and optional MIME type.
    pub const fn new(source: ResourceSource, media_type: Option<MediaType>) -> Self {
        Self { source, media_type }
    }

    /// Returns the resource source.
    pub fn source(&self) -> &ResourceSource {
        &self.source
    }

    /// Returns the declared media type, when present.
    pub fn media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }
}

/// File resource with optional declared MIME type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileResource {
    source: ResourceSource,
    media_type: Option<MediaType>,
}

impl FileResource {
    /// Creates a file resource from a source and optional MIME type.
    pub const fn new(source: ResourceSource, media_type: Option<MediaType>) -> Self {
        Self { source, media_type }
    }

    /// Returns the resource source.
    pub fn source(&self) -> &ResourceSource {
        &self.source
    }

    /// Returns the declared media type, when present.
    pub fn media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }
}

/// Portable resource kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    /// Image input.
    Image,
    /// Audio input.
    Audio,
    /// File input.
    File,
}

/// A typed image, audio, or file resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resource {
    /// Image input resource.
    Image(ImageResource),
    /// Audio input resource.
    Audio(AudioResource),
    /// File input resource.
    File(FileResource),
}

impl Resource {
    /// Returns the resource kind without inspecting its source.
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Image(_) => ResourceKind::Image,
            Self::Audio(_) => ResourceKind::Audio,
            Self::File(_) => ResourceKind::File,
        }
    }

    /// Returns the contained source.
    pub fn source(&self) -> &ResourceSource {
        match self {
            Self::Image(value) => value.source(),
            Self::Audio(value) => value.source(),
            Self::File(value) => value.source(),
        }
    }

    /// Returns the declared media type, when present.
    pub fn media_type(&self) -> Option<&MediaType> {
        match self {
            Self::Image(value) => value.media_type(),
            Self::Audio(value) => value.media_type(),
            Self::File(value) => value.media_type(),
        }
    }

    /// Returns the image-detail hint for image resources.
    pub const fn image_detail(&self) -> Option<ImageDetail> {
        match self {
            Self::Image(value) => value.detail(),
            Self::Audio(_) | Self::File(_) => None,
        }
    }

    /// Creates an image resource.
    pub const fn image(
        source: ResourceSource,
        media_type: Option<MediaType>,
        detail: Option<ImageDetail>,
    ) -> Self {
        Self::Image(ImageResource::new(source, media_type, detail))
    }

    /// Creates an audio resource.
    pub const fn audio(source: ResourceSource, media_type: Option<MediaType>) -> Self {
        Self::Audio(AudioResource::new(source, media_type))
    }

    /// Creates a file resource.
    pub const fn file(source: ResourceSource, media_type: Option<MediaType>) -> Self {
        Self::File(FileResource::new(source, media_type))
    }
}
