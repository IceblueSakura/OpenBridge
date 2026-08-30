//! Owned semantic leaf values shared by static Generation request and response IR.
//!
//! These values validate only their local shape and caller-supplied bounds. They do not perform
//! I/O, resolve Providers, or retain Route, credential, or transport state. Aggregate limits belong
//! to ingress and requirements projection.

use std::fmt;

use thiserror::Error;

/// Local validation failure for a semantic leaf value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SemanticValidationError {
    /// A value that must identify something is empty.
    #[error("{kind} must not be empty")]
    Empty {
        /// Human-readable value category.
        kind: &'static str,
    },
    /// A value exceeds its caller-supplied UTF-8 or encoded-byte bound.
    #[error("{kind} exceeds the {max_bytes}-byte limit")]
    TooLarge {
        /// Human-readable value category.
        kind: &'static str,
        /// Maximum accepted bytes.
        max_bytes: usize,
    },
    /// A URL is not an absolute URL.
    #[error("invalid URL")]
    InvalidUrl,
    /// A media type does not have a valid `type/subtype` shape.
    #[error("invalid media type")]
    InvalidMediaType,
    /// A bounded JSON value exceeds its encoded-byte bound.
    #[error("opaque JSON exceeds the {max_bytes}-byte limit")]
    OpaqueJsonTooLarge {
        /// Maximum accepted encoded JSON bytes.
        max_bytes: usize,
    },
    /// The JSON value supplied for an opaque JSON extension is not an object.
    #[error("opaque JSON must be an object")]
    InvalidOpaqueJson,
    /// The requested output-token limit is zero.
    #[error("maximum output tokens must be greater than zero")]
    ZeroOutputLimit,
    /// The requested candidate count is zero.
    #[error("candidate count must be greater than zero")]
    ZeroCandidateCount,
    /// A floating-point generation control is NaN or infinite.
    #[error("generation control must be finite")]
    NonFiniteControl,
    /// A Provider resource reference has no replay origin.
    #[error("Provider resource reference requires an origin")]
    MissingProviderOrigin,
    /// An inline media/file resource contains no bytes.
    #[error("inline resource must not be empty")]
    EmptyInlineResource,
    /// A Provider origin belongs to a different namespace than the value it owns.
    #[error("Provider origin namespace does not match semantic value namespace")]
    OriginNamespaceMismatch,
    /// Continuation state uses a non-continuation opaque kind.
    #[error("request continuation must use the continuation opaque kind")]
    InvalidContinuationKind,
    /// Provider resource reference uses a non-extension opaque kind.
    #[error("Provider resource reference must use the extension opaque kind")]
    InvalidProviderReferenceKind,
}

pub(super) fn bounded_string(
    value: impl Into<String>,
    kind: &'static str,
    max_bytes: usize,
) -> Result<String, SemanticValidationError> {
    let value = value.into();
    if value.is_empty() {
        return Err(SemanticValidationError::Empty { kind });
    }
    if value.len() > max_bytes {
        return Err(SemanticValidationError::TooLarge { kind, max_bytes });
    }
    Ok(value)
}

/// Owned bytes constrained by a bound supplied by the owning decoder or aggregate.
///
/// The bound is intentionally not stored: it is an admission rule, not semantic data. Empty bytes
/// are allowed because event deltas and Provider opaque payloads may be empty at a local boundary;
/// resource constructors can impose stronger rules when needed.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct BoundedBytes(Vec<u8>);

impl BoundedBytes {
    /// Creates bytes after checking the caller-supplied encoded-byte bound.
    pub fn new(
        value: impl Into<Vec<u8>>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        let value = value.into();
        if value.len() > max_bytes {
            return Err(SemanticValidationError::TooLarge {
                kind: "bytes",
                max_bytes,
            });
        }
        Ok(Self(value))
    }

    /// Creates bounded bytes by copying a slice.
    pub fn from_slice(value: &[u8], max_bytes: usize) -> Result<Self, SemanticValidationError> {
        Self::new(value.to_vec(), max_bytes)
    }

    /// Returns the owned bytes as a slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Returns the byte length.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether no bytes are present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Consumes the wrapper and returns its bytes.
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

/// An absolute URL retained as its original bounded string.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UrlValue(String);

impl UrlValue {
    /// Creates a URL after checking its syntax and UTF-8 byte bound.
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        let value = bounded_string(value, "URL", max_bytes)?;
        let parsed = url::Url::parse(&value).map_err(|_| SemanticValidationError::InvalidUrl)?;
        if parsed.scheme().is_empty() {
            return Err(SemanticValidationError::InvalidUrl);
        }
        Ok(Self(value))
    }

    /// Returns the validated URL string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the URL string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for UrlValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A bounded, syntactically valid MIME media type.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MediaType(String);

impl MediaType {
    /// Creates a media type with one non-empty type and subtype component.
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        let value = bounded_string(value, "media type", max_bytes)?;
        let mut components = value.split('/');
        let top = components.next().unwrap_or_default();
        let subtype = components.next().unwrap_or_default();
        if components.next().is_some()
            || top.is_empty()
            || subtype.is_empty()
            || top
                .chars()
                .any(|character| character.is_ascii_whitespace() || character.is_control())
            || subtype
                .chars()
                .any(|character| character.is_ascii_whitespace() || character.is_control())
        {
            return Err(SemanticValidationError::InvalidMediaType);
        }
        Ok(Self(value))
    }

    /// Returns the validated media type.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the type component before `/`.
    pub fn type_name(&self) -> &str {
        self.0
            .split_once('/')
            .map_or(self.0.as_str(), |(kind, _)| kind)
    }

    /// Returns the subtype component after `/`.
    pub fn subtype(&self) -> &str {
        self.0.split_once('/').map_or("", |(_, subtype)| subtype)
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
