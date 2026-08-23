//! Retry and error-category policy shared by generation forwarding.

use http::StatusCode;

use crate::{
    observability::{AttemptFailure, ErrorType, NextAction},
    provider::{ProviderAdapter, RetryHint, UpstreamErrorKind},
    transport::upstream::TransportError,
};

/// Returns whether a status permits continuing the current attempt before the first downstream event.
pub(super) fn should_retry_status(adapter: &ProviderAdapter, status: StatusCode) -> bool {
    adapter.classify_status(status).retry_hint() == RetryHint::BeforeFirstEvent
}

/// Includes only timeout/request transport failures that can be safely resent in retry.
pub(super) fn should_retry_error(error: &TransportError) -> bool {
    matches!(
        error,
        TransportError::Timeout | TransportError::Request(_) | TransportError::ResponseBody
    )
}

/// Maps transport errors to low-cardinality observation categories without underlying messages.
pub(super) fn transport_error_type(error: &TransportError) -> ErrorType {
    match error {
        TransportError::ClientBuild(_) => ErrorType::TransportClientBuild,
        TransportError::Request(_) => ErrorType::TransportRequest,
        TransportError::Timeout => ErrorType::Timeout,
        TransportError::ResponseBody => ErrorType::UpstreamBodyTransport,
        TransportError::InvalidTarget => ErrorType::InvalidTarget,
    }
}

/// Builds one closed HTTP-failure observation after ingress chooses the actual next action.
pub(super) fn http_attempt_failure(
    adapter: &ProviderAdapter,
    status: StatusCode,
    next_action: NextAction,
) -> AttemptFailure {
    let classification = adapter.classify_status(status);
    let error_type = match classification.kind() {
        UpstreamErrorKind::InvalidRequest => ErrorType::UpstreamInvalidRequest,
        UpstreamErrorKind::Authentication => ErrorType::UpstreamAuthentication,
        UpstreamErrorKind::RateLimited => ErrorType::UpstreamRateLimited,
        UpstreamErrorKind::UpstreamUnavailable => ErrorType::UpstreamUnavailable,
        UpstreamErrorKind::UpstreamFailure => ErrorType::UpstreamFailure,
    };
    AttemptFailure::new(
        error_type,
        classification.retry_hint() == RetryHint::BeforeFirstEvent,
        next_action,
    )
}

/// Builds one closed transport-failure observation after ingress chooses the actual next action.
pub(super) fn transport_attempt_failure(
    error: &TransportError,
    next_action: NextAction,
) -> AttemptFailure {
    AttemptFailure::new(
        transport_error_type(error),
        should_retry_error(error),
        next_action,
    )
}
