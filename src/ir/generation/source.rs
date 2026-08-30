//! Portable source, citation, and annotated-text values.

use std::fmt;

use super::{
    IdentityValidationError, ItemId, ProviderExtension, ProviderReference, TextValue, UrlValue,
    WireIdentity,
};

/// Canonical identity for one source/citation target.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceId(String);

impl SourceId {
    /// Creates a non-empty source identity within the supplied UTF-8 byte bound.
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, IdentityValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdentityValidationError::Empty);
        }
        if value.len() > max_bytes {
            return Err(IdentityValidationError::TooLarge { max_bytes });
        }
        Ok(Self(value))
    }

    /// Returns the canonical source identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Reference from an annotation or tool output to one canonical source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRef {
    id: SourceId,
}

impl SourceRef {
    /// Creates a source reference.
    pub const fn new(id: SourceId) -> Self {
        Self { id }
    }

    /// Returns the referenced source identity.
    pub const fn id(&self) -> &SourceId {
        &self.id
    }
}

/// Portable text annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextAnnotation {
    /// Citation targeting a separate source item.
    Citation(SourceRef),
}

/// Text and its ordered portable annotations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextContent {
    text: TextValue,
    annotations: Vec<TextAnnotation>,
}

impl TextContent {
    /// Creates text content while preserving annotation order.
    pub fn new(text: TextValue, annotations: Vec<TextAnnotation>) -> Self {
        Self { text, annotations }
    }

    /// Returns the validated text value.
    pub const fn text(&self) -> &TextValue {
        &self.text
    }

    /// Returns ordered portable annotations.
    pub fn annotations(&self) -> &[TextAnnotation] {
        &self.annotations
    }
}

impl From<TextValue> for TextContent {
    fn from(text: TextValue) -> Self {
        Self::new(text, Vec::new())
    }
}

/// Portable source location.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceLocation {
    /// Public URL retained without fetching.
    Url(UrlValue),
    /// Provider-private source reference subject to origin affinity.
    ProviderReference(ProviderReference),
    /// Provider-private source metadata accepted only by an explicit target profile.
    Extension(ProviderExtension),
}

impl Eq for SourceLocation {}

/// One ordered response source item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    item_id: ItemId,
    id: SourceId,
    title: Option<TextValue>,
    location: SourceLocation,
    wire_identity: Option<WireIdentity>,
}

impl Source {
    /// Creates a source item from validated leaf values.
    pub fn new(
        item_id: ItemId,
        id: SourceId,
        title: Option<TextValue>,
        location: SourceLocation,
        wire_identity: Option<WireIdentity>,
    ) -> Self {
        Self {
            item_id,
            id,
            title,
            location,
            wire_identity,
        }
    }

    /// Returns the canonical output-item identity.
    pub const fn item_id(&self) -> &ItemId {
        &self.item_id
    }

    /// Returns the citation identity.
    pub const fn id(&self) -> &SourceId {
        &self.id
    }

    /// Returns the optional source title.
    pub const fn title(&self) -> Option<&TextValue> {
        self.title.as_ref()
    }

    /// Returns the portable source location.
    pub const fn location(&self) -> &SourceLocation {
        &self.location
    }

    /// Returns the Provider wire identity when one exists.
    pub const fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}
