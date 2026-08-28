//! Embeddings request analysis and fixed-interface planning errors.

use thiserror::Error;

/// Stable classification for Embeddings request analysis and fixed-interface planning failures.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum EmbeddingRequestError {
    /// The request syntax or one standard field value is invalid.
    #[error("embedding request is invalid")]
    InvalidRequest {
        /// Standard request field that uniquely locates the error, when available.
        param: Option<&'static str>,
    },
    /// The requested Public Model is absent, retired, or otherwise unavailable.
    #[error("requested embedding model is not available")]
    ModelNotFound,
    /// The selected Public Model's fixed Embeddings interface cannot satisfy one request field.
    #[error("selected model does not support the requested embedding capability")]
    UnsupportedModelCapability {
        /// Standard request field whose requirement exceeds the fixed interface.
        param: &'static str,
    },
    /// A compiled Embeddings interface cannot resolve its required single Native Route.
    #[error("configured embedding route is unavailable")]
    RouteUnavailable,
}

impl EmbeddingRequestError {
    /// Creates an invalid-request classification with an optional standard field location.
    pub(in crate::pipeline) const fn invalid(param: Option<&'static str>) -> Self {
        Self::InvalidRequest { param }
    }

    /// Creates a fixed-capability rejection located at one standard request field.
    pub(in crate::pipeline) const fn unsupported(param: &'static str) -> Self {
        Self::UnsupportedModelCapability { param }
    }
}
