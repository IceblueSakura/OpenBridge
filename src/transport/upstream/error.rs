//! Errors raised by the shared upstream HTTP transport.
//!
//! The transport reports client construction, request, timeout, and adapter-target failures;
//! higher layers classify HTTP responses and choose retry or fallback behavior.

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
