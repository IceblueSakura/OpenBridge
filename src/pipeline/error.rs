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
}
