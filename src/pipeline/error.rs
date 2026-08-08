//! Stable error types for request analysis and Route planning.

use thiserror::Error;

/// Planning error returned when a request fails Public Model preflight or cannot bind to a configured Route.
#[derive(Debug, Error)]
pub enum RequestPlanningError {
    /// The request body is not a JSON object.
    #[error("request body must be a JSON object")]
    InvalidJson,
    /// The request lacks a non-empty Public Model.
    #[error("request body must contain a non-empty model")]
    MissingModel,
    /// The requested Public Model is not registered.
    #[error("requested model is not configured")]
    UnknownModel,
    /// The Public Model has no statically executable Route.
    #[error("configured model has no executable route")]
    NoRoute,
    /// The Public Model has no fixed interface for the request protocol.
    #[error("selected model does not support this protocol")]
    UnsupportedProtocol,
    /// The Public Model's fixed interface does not support streaming.
    #[error("selected model does not support streaming")]
    StreamingUnsupported,
    /// The Public Model's fixed interface cannot deliver one complete non-streaming JSON response.
    #[error("selected model does not support non-streaming responses")]
    NonStreamingUnsupported,
    /// The Public Model's fixed interface does not support the requested capability.
    #[error("selected model does not support requested capabilities")]
    UnsupportedCapabilities,
    /// The request uses a named but unimplemented reserved capability.
    #[error("requested capabilities are reserved but not implemented")]
    UnimplementedCapabilities,
    /// The requested maximum output exceeds the effective limit.
    #[error("requested maximum output exceeds the configured model limit")]
    OutputLimitExceeded,
    /// The model does not support the requested reasoning.
    #[error("selected model does not support requested reasoning")]
    ReasoningUnsupported,
    /// The model does not support the requested reasoning level.
    #[error("selected model does not support the requested reasoning level")]
    ReasoningLevelUnsupported,
    /// The request provides conflicting reasoning configuration sources or shapes.
    #[error("request contains conflicting reasoning configuration")]
    InvalidReasoningConfiguration,
    /// A multimodal content part is malformed or appears outside its protocol-defined position.
    #[error("request contains invalid multimodal input")]
    InvalidMultimodalInput,
    /// Locally countable multimodal input exceeds a checked arithmetic or interface limit.
    #[error("request multimodal input exceeds the configured limit")]
    MultimodalInputLimitExceeded,
}

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
    pub(super) const fn invalid(param: Option<&'static str>) -> Self {
        Self::InvalidRequest { param }
    }

    /// Creates a fixed-capability rejection located at one standard request field.
    pub(super) const fn unsupported(param: &'static str) -> Self {
        Self::UnsupportedModelCapability { param }
    }
}
