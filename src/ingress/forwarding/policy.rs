//! Retry and error-category policy shared by generation forwarding.

use http::StatusCode;

use crate::{
    provider::{ProviderAdapter, RetryHint},
    transport::upstream::TransportError,
};

/// Returns whether a status permits continuing the current attempt before the first downstream event.
pub(super) fn should_retry_status(adapter: &ProviderAdapter, status: StatusCode) -> bool {
    adapter.classify_status(status).retry_hint() == RetryHint::BeforeFirstEvent
}

/// Includes only timeout/request transport failures that can be safely resent in retry.
pub(super) fn should_retry_error(error: &TransportError) -> bool {
    matches!(error, TransportError::Timeout | TransportError::Request(_))
}

/// Maps transport errors to low-cardinality observation categories without underlying messages.
pub(super) fn transport_error_kind(error: &TransportError) -> &'static str {
    match error {
        TransportError::ClientBuild(_) => "client_build",
        TransportError::Request(_) => "request",
        TransportError::Timeout => "timeout",
        TransportError::InvalidTarget => "invalid_target",
    }
}
