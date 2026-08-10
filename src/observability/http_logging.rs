//! Local rendering of explicitly enabled downstream HTTP header and body snapshots.
//!
//! Events stay on the local tracing formatter because the OTLP layer exports spans only. Header
//! names and safe values remain visible, while authentication, Cookie, token, key, secret, and
//! password-like header values are always replaced before entering tracing fields.

use http::{HeaderMap, HeaderValue, Method, StatusCode};
use tracing::Span;

const REDACTED: &str = "[REDACTED]";

/// Emits one sanitized snapshot of authenticated downstream request headers.
pub(super) fn emit_request_headers(span: &Span, method: &Method, path: &str, headers: &HeaderMap) {
    // Materialize only the opted-in snapshot so disabled requests allocate no header copy.
    let headers = sanitized_headers(headers);

    // Keep the event inside the request span for local correlation without OTLP event export.
    span.in_scope(|| {
        tracing::info!(%method, %path, headers = ?headers, "downstream_request_headers");
    });
}

/// Emits one sanitized snapshot of downstream response headers.
pub(super) fn emit_response_headers(span: &Span, status: StatusCode, headers: &HeaderMap) {
    // Materialize only the opted-in snapshot so disabled responses allocate no header copy.
    let headers = sanitized_headers(headers);

    // Keep the event inside the request span for local correlation without OTLP event export.
    span.in_scope(|| {
        tracing::info!(status = status.as_u16(), headers = ?headers, "downstream_response_headers");
    });
}

/// Emits one bounded request-body snapshot at EOF, error, or cancellation.
pub(super) fn emit_request_body(
    span: &Span,
    bytes: &[u8],
    total_bytes: usize,
    complete: bool,
    truncated: bool,
) {
    emit_body(span, "request", bytes, total_bytes, complete, truncated);
}

/// Emits one bounded response-body snapshot at EOF, error, or cancellation.
pub(super) fn emit_response_body(
    span: &Span,
    bytes: &[u8],
    total_bytes: usize,
    complete: bool,
    truncated: bool,
) {
    emit_body(span, "response", bytes, total_bytes, complete, truncated);
}

/// Emits a body using escaped UTF-8 text and explicit capture completeness metadata.
fn emit_body(
    span: &Span,
    message: &'static str,
    bytes: &[u8],
    total_bytes: usize,
    complete: bool,
    truncated: bool,
) {
    // Preserve valid text exactly and replace malformed byte sequences without terminal control injection.
    let encoding = if std::str::from_utf8(bytes).is_ok() {
        "utf-8"
    } else {
        "utf-8-lossy"
    };
    let content = String::from_utf8_lossy(bytes);

    // Emit one terminal snapshot instead of producing a log event for every body chunk.
    span.in_scope(|| match message {
        "request" => tracing::info!(
            body_encoding = encoding,
            captured_bytes = bytes.len(),
            observed_bytes = total_bytes,
            complete,
            truncated,
            content = ?content,
            "downstream_request_body"
        ),
        "response" => tracing::info!(
            body_encoding = encoding,
            captured_bytes = bytes.len(),
            observed_bytes = total_bytes,
            complete,
            truncated,
            content = ?content,
            "downstream_response_body"
        ),
        _ => unreachable!("local HTTP body message kind must be closed"),
    });
}

/// Copies all header names and values while replacing sensitive values before formatting.
fn sanitized_headers(headers: &HeaderMap) -> Vec<(String, Vec<String>)> {
    // Preserve duplicate values under each normalized HTTP header name.
    headers
        .keys()
        .map(|name| {
            let values = headers
                .get_all(name)
                .iter()
                .map(|value| {
                    if is_sensitive_header(name.as_str()) {
                        REDACTED.to_owned()
                    } else {
                        render_header_value(value)
                    }
                })
                .collect();
            (name.as_str().to_owned(), values)
        })
        .collect()
}

/// Renders every non-sensitive header byte, escaping values that are not valid UTF-8.
fn render_header_value(value: &HeaderValue) -> String {
    // Keep ordinary text readable while preserving invalid UTF-8 as reversible ASCII escapes.
    value.to_str().map(str::to_owned).unwrap_or_else(|_| {
        value
            .as_bytes()
            .iter()
            .flat_map(|byte| std::ascii::escape_default(*byte))
            .map(char::from)
            .collect()
    })
}

/// Returns whether a header name can carry authentication or secret material.
fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name,
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
    ) || name.contains("auth")
        || name.contains("apikey")
        || name.contains("key")
        || name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("passwd")
        || name.contains("session")
        || name.contains("credential")
        || name.contains("signature")
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue};

    use super::sanitized_headers;

    #[test]
    fn header_snapshot_preserves_safe_duplicates_and_redacts_secret_like_names() {
        let mut headers = HeaderMap::new();
        headers.append("x-safe", HeaderValue::from_static("first"));
        headers.append("x-safe", HeaderValue::from_static("second"));
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer synthetic-secret"),
        );
        headers.insert("x-api-key", HeaderValue::from_static("synthetic-key"));
        headers.insert(
            "x-binary-debug",
            HeaderValue::from_bytes(b"visible-\x80-byte").unwrap(),
        );

        // Preserve useful repeated values without copying any sensitive value into the snapshot.
        let snapshot = sanitized_headers(&headers);
        assert!(snapshot.contains(&(
            "x-safe".to_owned(),
            vec!["first".to_owned(), "second".to_owned()]
        )));
        assert!(snapshot.contains(&("authorization".to_owned(), vec!["[REDACTED]".to_owned()])));
        assert!(snapshot.contains(&("x-api-key".to_owned(), vec!["[REDACTED]".to_owned()])));
        assert!(snapshot.contains(&(
            "x-binary-debug".to_owned(),
            vec![r"visible-\x80-byte".to_owned()]
        )));
        assert!(!format!("{snapshot:?}").contains("synthetic-secret"));
        assert!(!format!("{snapshot:?}").contains("synthetic-key"));
    }
}
