//! Safe header, SSE, and error-classification contracts used by Provider adapters.
//!
//! `SafeHeaders` and `SensitiveHeaders` are intentionally separate: the former cannot carry
//! authentication, host, or cookie headers; the latter is converted to HTTP headers marked
//! sensitive only before egress and zeroes its strings on drop.

use std::{collections::HashMap, fmt};

use http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{AUTHORIZATION, COOKIE, HOST, PROXY_AUTHORIZATION},
};
use zeroize::Zeroizing;

use crate::transport::sse::SseEvent;

use super::AdapterError;

/// Non-sensitive request headers that trusted Provider hooks may add, change, or remove.
///
/// Authentication, cookie, host, and proxy-authorization headers cannot be written through this type.
#[derive(Default)]
pub struct SafeHeaders(HeaderMap);

impl SafeHeaders {
    /// Reads an allowed request header.
    pub fn get(&self, name: HeaderName) -> Option<&HeaderValue> {
        self.0.get(name)
    }

    /// Writes or replaces an ordinary request header.
    ///
    /// Rejects authentication, cookie, Host, and proxy-authentication headers.
    pub fn insert(&mut self, name: HeaderName, value: HeaderValue) -> Result<(), AdapterError> {
        // Reject authentication, cookie, host, and proxy authorization to preserve the ordinary-header boundary.
        if name == AUTHORIZATION || name == PROXY_AUTHORIZATION || name == COOKIE || name == HOST {
            return Err(AdapterError::SensitiveHeaderInSafeSet);
        }
        self.0.insert(name, value);
        Ok(())
    }

    /// Removes a request header and returns its previous value.
    pub fn remove(&mut self, name: HeaderName) -> Option<HeaderValue> {
        self.0.remove(name)
    }

    /// Consumes the safe-header set for egress to assemble the final request.
    pub(crate) fn into_inner(self) -> HeaderMap {
        self.0
    }
}

impl fmt::Debug for SafeHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SafeHeaders")
            .field("header_names", &self.0.keys().collect::<Vec<_>>())
            .field("values", &"[OMITTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderName, HeaderValue};

    use super::*;

    #[test]
    fn safe_headers_support_regular_header_rewrite_and_drop() {
        let source = HeaderName::from_static("x-provider-source");
        let target = HeaderName::from_static("x-provider-target");
        let mut headers = SafeHeaders::default();

        headers
            .insert(source.clone(), HeaderValue::from_static("source-value"))
            .unwrap();
        headers
            .insert(
                target.clone(),
                HeaderValue::from_static("transformed-value"),
            )
            .unwrap();
        headers.remove(source.clone());

        assert!(headers.get(source).is_none());
        assert_eq!(headers.get(target).unwrap(), "transformed-value");
    }
}

/// Sensitive request headers added only before upstream send and hidden in debug output.
#[derive(Default)]
pub struct SensitiveHeaders(HashMap<HeaderName, Zeroizing<String>>);

impl SensitiveHeaders {
    /// Returns whether the set contains the named header.
    pub fn contains(&self, name: HeaderName) -> bool {
        self.0.contains_key(&name)
    }

    /// Reads a sensitive header only in crate tests; production code has no such access path.
    #[cfg(test)]
    pub(super) fn expose(&self, name: HeaderName) -> Option<&str> {
        self.0.get(&name).map(|value| value.as_str())
    }

    /// Stores a sensitive header value still held by a zeroizing container.
    pub(crate) fn insert(&mut self, name: HeaderName, value: Zeroizing<String>) {
        self.0.insert(name, value);
    }

    /// Converts the sensitive value into a marked HTTP header and consumes the temporary container.
    pub(crate) fn append_to(self, headers: &mut HeaderMap) -> Result<(), AdapterError> {
        // Convert the sensitive string once, mark the HTTP header sensitive, and release the source container.
        for (name, value) in self.0 {
            let mut value = HeaderValue::from_str(value.as_str())
                .map_err(|_| AdapterError::InvalidAuthenticationHeader)?;
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Ok(())
    }
}

impl fmt::Debug for SensitiveHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SensitiveHeaders")
            .field("header_names", &self.0.keys().collect::<Vec<_>>())
            .field("values", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Lifecycle state of an upstream SSE event in ingress.
pub enum StreamEventStatus {
    /// The event is not terminal; continue reading upstream.
    Continue,
    /// The event represents normal completion.
    Completed,
    /// The event represents a Provider-side failure.
    Failed,
}

/// Fully framed SSE event not yet rewritten by ingress.
#[derive(Debug)]
pub struct ClassifiedSseEvent {
    event: SseEvent,
    status: StreamEventStatus,
}

impl ClassifiedSseEvent {
    /// Creates an SSE event with completed framing and classified lifecycle.
    pub(crate) fn new(event: SseEvent, status: StreamEventStatus) -> Self {
        Self { event, status }
    }

    /// Returns the raw SSE event.
    pub fn event(&self) -> &SseEvent {
        &self.event
    }

    /// Returns the adapter's lifecycle classification for the event.
    pub fn status(&self) -> StreamEventStatus {
        self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Coarse category for an upstream HTTP failure.
pub enum UpstreamErrorKind {
    /// The request content or parameters are invalid.
    InvalidRequest,
    /// The upstream rejected authentication.
    Authentication,
    /// The upstream rate-limited the request.
    RateLimited,
    /// The upstream is temporarily unavailable.
    UpstreamUnavailable,
    /// Another upstream failure.
    UpstreamFailure,
}

/// Retry boundary reported by an adapter for an HTTP status, not an instruction to always retry.
///
/// Ingress also applies streaming, attempt limits, candidate order, and downstream-output conditions;
/// `BeforeFirstEvent` can never be used to append to a token stream that has already started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryHint {
    /// Automatic retry is not allowed for this status.
    Never,
    /// Retry is allowed only before the first event is sent downstream.
    BeforeFirstEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Error category and retry boundary reported by an adapter for an upstream status.
pub struct StatusClassification {
    kind: UpstreamErrorKind,
    retry_hint: RetryHint,
}

impl StatusClassification {
    /// Creates a status result bound to an error category and earliest retry boundary.
    pub(crate) fn new(kind: UpstreamErrorKind, retry_hint: RetryHint) -> Self {
        Self { kind, retry_hint }
    }

    /// Returns the upstream error category.
    pub fn kind(&self) -> UpstreamErrorKind {
        self.kind
    }

    /// Returns the earliest retry boundary suggested by the adapter.
    pub fn retry_hint(&self) -> RetryHint {
        self.retry_hint
    }
}
