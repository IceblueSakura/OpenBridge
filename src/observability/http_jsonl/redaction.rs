//! Header redaction for JSONL snapshot records.

use http::HeaderMap;

use super::record::HeaderEntry;

fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
    ) || lower.contains("auth")
        || lower.contains("apikey")
        || lower.contains("key")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("passwd")
        || lower.contains("session")
        || lower.contains("credential")
        || lower.contains("signature")
}

pub(crate) fn redact_headers(headers: &HeaderMap) -> Vec<HeaderEntry> {
    headers
        .iter()
        .map(|(name, value)| HeaderEntry {
            name: name.as_str().to_owned(),
            value: if is_sensitive_header(name.as_str()) {
                "[REDACTED]".to_owned()
            } else {
                value
                    .to_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|_| "[NON_UTF8]".to_owned())
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue};

    use super::redact_headers;

    #[test]
    fn redacts_arbitrary_secret_like_header_names() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-vendor-session-signature",
            HeaderValue::from_static("synthetic"),
        );
        assert_eq!(redact_headers(&headers)[0].value, "[REDACTED]");
    }
}
