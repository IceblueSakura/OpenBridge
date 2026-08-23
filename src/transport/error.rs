//! Errors raised by the shared upstream HTTP transport.
//!
//! The transport reports client construction, request, timeout, and adapter-target failures;
//! higher layers classify HTTP responses and choose retry or fallback behavior.

use std::{error::Error as StdError, io};

use thiserror::Error;

/// Error reported by upstream transport while building the client, sending a request, or enforcing timeouts.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The reqwest client cannot be built using bootstrap policy.
    #[error("failed to construct the upstream HTTP client")]
    ClientBuild(#[source] reqwest::Error),
    /// The upstream request failed while sending or receiving.
    #[error("upstream request failed")]
    Request(#[source] reqwest::Error),
    /// The upstream request exceeded the target timeout.
    #[error("upstream request timed out")]
    Timeout,
    /// The adapter generated a URI with an authority, scheme, or invalid path.
    #[error("provider adapter produced an invalid relative upstream target")]
    InvalidTarget,
}

/// Detects only typed timeout causes in a body error chain without retaining error strings.
pub(crate) fn is_timeout_error(error: &(dyn StdError + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::TimedOut)
            || error
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_timeout)
        {
            return true;
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::is_timeout_error;

    #[test]
    fn timeout_detection_uses_typed_error_causes() {
        let timeout = io::Error::new(io::ErrorKind::TimedOut, "synthetic timeout");
        let other = io::Error::new(io::ErrorKind::ConnectionReset, "synthetic reset");

        assert!(is_timeout_error(&timeout));
        assert!(!is_timeout_error(&other));
    }
}
