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

#[cfg(test)]
mod tests {
    //! Verifies strict downstream Bearer-header parsing without exposing token values.

    use http::{HeaderMap, HeaderValue, header::AUTHORIZATION};

    use super::bearer_token;

    #[test]
    fn bearer_parser_accepts_the_exact_scheme_and_rejects_malformed_values() {
        // Accept one non-empty token with the exact configured Bearer scheme.
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer synthetic-token"),
        );
        assert_eq!(bearer_token(&headers), Some("synthetic-token"));

        // Reject empty, differently cased, and unrelated authorization schemes.
        for value in ["Bearer ", "bearer synthetic-token", "Basic synthetic-token"] {
            headers.insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
            assert_eq!(bearer_token(&headers), None);
        }

        // Reject non-text header bytes and an absent header without exposing their contents.
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_bytes(b"Bearer \xff").unwrap(),
        );
        assert_eq!(bearer_token(&headers), None);
        headers.remove(AUTHORIZATION);
        assert_eq!(bearer_token(&headers), None);
    }
}
