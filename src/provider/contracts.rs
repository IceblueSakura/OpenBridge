//! Safe header, SSE, and error-classification contracts used by Provider adapters.
//!
//! `SafeHeaders` and `SensitiveHeaders` are intentionally separate: the former cannot carry
//! authentication, host, or cookie headers; the latter is converted to HTTP headers marked
//! sensitive only before egress and zeroes its strings on drop.

use std::{collections::HashMap, fmt};

use http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{AUTHORIZATION, COOKIE, HOST, PROXY_AUTHORIZATION, USER_AGENT},
};
use zeroize::Zeroizing;

use crate::transport::sse::SseEvent;

use super::AdapterError;

/// One fixed non-sensitive request header declared by trusted Provider code.
///
/// Names and values are parsed before egress. Provider definitions must not place secrets in this
/// type; authentication and account-routing material belongs in `SensitiveHeaders`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticRequestHeader {
    name: &'static str,
    value: &'static str,
}

impl StaticRequestHeader {
    /// Creates a fixed ordinary header from code-owned static strings.
    pub(crate) const fn new(name: &'static str, value: &'static str) -> Self {
        Self { name, value }
    }
}

/// Compile-time fixed `User-Agent` and ordinary request headers for one Provider adapter.
///
/// The profile is applied after the Provider's downstream-header hook so a business request cannot
/// override fixed identity. `SafeHeaders` still rejects credential-bearing header names.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProviderRequestHeaders {
    user_agent: Option<&'static str>,
    headers: &'static [StaticRequestHeader],
}

impl ProviderRequestHeaders {
    /// Creates an empty fixed-header profile.
    pub(crate) const fn new() -> Self {
        Self {
            user_agent: None,
            headers: &[],
        }
    }

    /// Sets the fixed Provider `User-Agent` value.
    pub(crate) const fn with_user_agent(mut self, user_agent: &'static str) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Sets the fixed ordinary headers declared by the Provider.
    pub(crate) const fn with_headers(mut self, headers: &'static [StaticRequestHeader]) -> Self {
        self.headers = headers;
        self
    }

    /// Parses and applies the fixed profile without exposing header values in errors.
    pub(crate) fn apply_to(self, target: &mut SafeHeaders) -> Result<(), AdapterError> {
        // Parse and apply each code-owned ordinary header through the sensitive-name guard.
        for header in self.headers {
            let name = HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| AdapterError::InvalidCompiledRequestHeader)?;
            let value = HeaderValue::from_str(header.value)
                .map_err(|_| AdapterError::InvalidCompiledRequestHeader)?;
            target.insert(name, value)?;
        }

        // Apply the dedicated User-Agent last so the explicit field wins over duplicate entries.
        if let Some(user_agent) = self.user_agent {
            if user_agent.is_empty() {
                return Err(AdapterError::InvalidCompiledRequestHeader);
            }
            let value = HeaderValue::from_str(user_agent)
                .map_err(|_| AdapterError::InvalidCompiledRequestHeader)?;
            target.insert(USER_AGENT, value)?;
        }
        Ok(())
    }
}

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
    use http::{HeaderName, header::USER_AGENT};

    use super::*;

    #[test]
    fn compiled_request_headers_apply_custom_values_and_fail_closed() {
        const ORDINARY_HEADERS: &[StaticRequestHeader] =
            &[StaticRequestHeader::new("x-provider-identity", "compiled")];
        const REQUEST_HEADERS: ProviderRequestHeaders = ProviderRequestHeaders::new()
            .with_user_agent("provider-client/1.0")
            .with_headers(ORDINARY_HEADERS);
        const FORBIDDEN_HEADERS: &[StaticRequestHeader] =
            &[StaticRequestHeader::new("authorization", "must-not-escape")];
        const INVALID_HEADERS: &[StaticRequestHeader] =
            &[StaticRequestHeader::new("invalid header name", "value")];

        // Apply a valid fixed User-Agent and ordinary header profile.
        let mut headers = SafeHeaders::default();
        REQUEST_HEADERS.apply_to(&mut headers).unwrap();

        assert_eq!(headers.get(USER_AGENT).unwrap(), "provider-client/1.0");
        assert_eq!(
            headers
                .get(HeaderName::from_static("x-provider-identity"))
                .unwrap(),
            "compiled"
        );

        // Reject both sensitive names and invalid HTTP metadata without exposing values.
        let error = ProviderRequestHeaders::new()
            .with_headers(FORBIDDEN_HEADERS)
            .apply_to(&mut SafeHeaders::default())
            .unwrap_err();
        assert!(matches!(error, AdapterError::SensitiveHeaderInSafeSet));
        let error = ProviderRequestHeaders::new()
            .with_headers(INVALID_HEADERS)
            .apply_to(&mut SafeHeaders::default())
            .unwrap_err();
        assert!(matches!(error, AdapterError::InvalidCompiledRequestHeader));
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
