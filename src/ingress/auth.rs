//! Parses downstream Bearer headers.

use http::{HeaderMap, header::AUTHORIZATION};

/// Extracts a non-empty Bearer token from the Authorization header without logging or copying its content.
pub(super) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}
