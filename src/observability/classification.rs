//! Closed, low-cardinality classifications shared by request and Provider observation.

use http::Method;

/// Authenticated endpoint family used to separate inference and control-plane traffic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestKind {
    Generation,
    Embeddings,
    Images,
    Models,
    Mcp,
}

impl RequestKind {
    /// Classifies only routes protected by the authenticated request-observation middleware.
    pub(crate) fn from_http(method: &Method, path: &str) -> Option<Self> {
        match (method, path) {
            (&Method::POST, "/v1/chat/completions" | "/v1/responses") => Some(Self::Generation),
            (&Method::POST, "/v1/embeddings") => Some(Self::Embeddings),
            (&Method::POST, "/v1/images/generations") => Some(Self::Images),
            (&Method::GET, path)
                if path == "/v1/models"
                    || path.starts_with("/v1/models/")
                    || path == "/openbridge/v1/models"
                    || path.starts_with("/openbridge/v1/models/") =>
            {
                Some(Self::Models)
            }
            (_, path) if path == "/mcp" || path.starts_with("/mcp/") => Some(Self::Mcp),
            _ => None,
        }
    }

    /// Returns the stable trace and metric value.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Generation => "generation",
            Self::Embeddings => "embeddings",
            Self::Images => "images",
            Self::Models => "models",
            Self::Mcp => "mcp",
        }
    }
}

/// Stable, bounded diagnostic cause shared by request and Provider attempt terminals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ErrorType {
    InvalidRequest,
    UnknownModel,
    UnimplementedCapability,
    UnsupportedCapability,
    ConfigurationError,
    UpstreamInvalidRequest,
    UpstreamAuthentication,
    UpstreamRateLimited,
    UpstreamUnavailable,
    UpstreamFailure,
    TransportClientBuild,
    TransportRequest,
    UpstreamBodyTransport,
    Timeout,
    InvalidTarget,
    InvalidUpstreamResponse,
    SseEofBeforeTerminal,
    ProviderTerminalFailed,
    DownstreamBodyError,
    ClientCancelled,
}

impl ErrorType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnknownModel => "unknown_model",
            Self::UnimplementedCapability => "unimplemented_capability",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::ConfigurationError => "configuration_error",
            Self::UpstreamInvalidRequest => "upstream_invalid_request",
            Self::UpstreamAuthentication => "upstream_authentication",
            Self::UpstreamRateLimited => "upstream_rate_limited",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::UpstreamFailure => "upstream_failure",
            Self::TransportClientBuild => "transport_client_build",
            Self::TransportRequest => "transport_request",
            Self::UpstreamBodyTransport => "upstream_body_transport",
            Self::Timeout => "timeout",
            Self::InvalidTarget => "invalid_target",
            Self::InvalidUpstreamResponse => "invalid_upstream_response",
            Self::SseEofBeforeTerminal => "sse_eof_before_terminal",
            Self::ProviderTerminalFailed => "provider_terminal_failed",
            Self::DownstreamBodyError => "downstream_body_error",
            Self::ClientCancelled => "client_cancelled",
        }
    }
}

/// Closed timeout phase retained without provider, URL, or request-content dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimeoutPhase {
    ResponseHeaders,
    FirstEvent,
    EventIdle,
    StreamTotal,
    NonStreamingTotal,
}

impl TimeoutPhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ResponseHeaders => "response_headers",
            Self::FirstEvent => "first_event",
            Self::EventIdle => "event_idle",
            Self::StreamTotal => "stream_total",
            Self::NonStreamingTotal => "non_stream_total",
        }
    }
}

/// Request-processing boundary at which a terminal cause was directly observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureStage {
    Analysis,
    Planning,
    Credential,
    Upstream,
    Stream,
    Bridge,
    DownstreamDelivery,
}

impl FailureStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::Planning => "planning",
            Self::Credential => "credential",
            Self::Upstream => "upstream",
            Self::Stream => "stream",
            Self::Bridge => "bridge",
            Self::DownstreamDelivery => "downstream_delivery",
        }
    }
}

/// Actual routing decision made after one failed physical attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NextAction {
    Finish,
    RetryCandidate,
    NextCandidate,
}

impl NextAction {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Finish => "finish",
            Self::RetryCandidate => "retry_candidate",
            Self::NextCandidate => "next_candidate",
        }
    }
}

/// Closed diagnostic context retained until one request reaches its terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RequestFailure {
    pub(crate) error_type: ErrorType,
    pub(crate) stage: FailureStage,
    pub(crate) retryable: bool,
    pub(crate) next_action: NextAction,
}

impl RequestFailure {
    pub(crate) const fn terminal(
        error_type: ErrorType,
        stage: FailureStage,
        retryable: bool,
    ) -> Self {
        Self {
            error_type,
            stage,
            retryable,
            next_action: NextAction::Finish,
        }
    }
}

/// Closed diagnostic context finalized with one failed physical Provider attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AttemptFailure {
    pub(crate) error_type: ErrorType,
    pub(crate) retryable: bool,
    pub(crate) next_action: NextAction,
}

impl AttemptFailure {
    pub(crate) const fn new(
        error_type: ErrorType,
        retryable: bool,
        next_action: NextAction,
    ) -> Self {
        Self {
            error_type,
            retryable,
            next_action,
        }
    }

    pub(crate) const fn request_failure(self, stage: FailureStage) -> RequestFailure {
        RequestFailure {
            error_type: self.error_type,
            stage,
            retryable: self.retryable,
            next_action: self.next_action,
        }
    }
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::{ErrorType, FailureStage, NextAction, RequestKind};

    #[test]
    fn authenticated_routes_have_closed_request_kinds() {
        for (method, path, expected) in [
            (
                Method::POST,
                "/v1/chat/completions",
                RequestKind::Generation,
            ),
            (Method::POST, "/v1/responses", RequestKind::Generation),
            (Method::POST, "/v1/embeddings", RequestKind::Embeddings),
            (Method::GET, "/v1/models/model-a", RequestKind::Models),
            (Method::GET, "/openbridge/v1/models", RequestKind::Models),
            (Method::POST, "/mcp", RequestKind::Mcp),
        ] {
            assert_eq!(RequestKind::from_http(&method, path), Some(expected));
        }
        assert_eq!(RequestKind::from_http(&Method::GET, "/healthz"), None);
    }

    #[test]
    fn diagnostic_classifications_have_stable_wire_names() {
        assert_eq!(
            ErrorType::UpstreamRateLimited.as_str(),
            "upstream_rate_limited"
        );
        assert_eq!(ErrorType::TransportRequest.as_str(), "transport_request");
        assert_eq!(FailureStage::Planning.as_str(), "planning");
        assert_eq!(FailureStage::Stream.as_str(), "stream");
        assert_eq!(NextAction::RetryCandidate.as_str(), "retry_candidate");
        assert_eq!(NextAction::NextCandidate.as_str(), "next_candidate");
    }
}
