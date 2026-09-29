//! Fixed provider identity, trusted origin and relative entry paths.

use crate::provider::{auth::AuthScheme, errors::ProviderError, ident_ok};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(id: &str) -> Result<Self, ProviderError> {
        if ident_ok(id) {
            Ok(Self(id.into()))
        } else {
            Err(ProviderError::InvalidId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Normalized `scheme://host[:port]` with no path, query, fragment or userinfo.
///
/// HTTPS is required except for loopback test origins, where plaintext HTTP is
/// contained to `localhost`/`127.0.0.1`/`[::1]`. Credentials in the URL are
/// rejected outright.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedOrigin(String);

impl TrustedOrigin {
    pub fn parse(s: &str) -> Result<Self, ProviderError> {
        let (scheme, rest) = s.split_once("://").ok_or(ProviderError::InvalidOrigin)?;
        let scheme = match scheme.to_ascii_lowercase().as_str() {
            "https" => "https",
            "http" => "http",
            _ => return Err(ProviderError::InvalidOrigin),
        };
        if rest.is_empty() || rest.contains(['/', '?', '#', '@', ' ']) {
            return Err(ProviderError::InvalidOrigin);
        }
        // IPv6 literals keep brackets; everything else is `host` or `host:port`.
        let (host, port) = if let Some(addr) = rest.strip_prefix('[') {
            let (inner, tail) = addr.split_once(']').ok_or(ProviderError::InvalidOrigin)?;
            if inner.is_empty()
                || !inner
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
            {
                return Err(ProviderError::InvalidOrigin);
            }
            let port = match tail {
                "" => None,
                _ => Some(tail.strip_prefix(':').ok_or(ProviderError::InvalidOrigin)?),
            };
            (format!("[{}]", inner.to_ascii_lowercase()), port)
        } else {
            match rest.split_once(':') {
                Some((host, port)) => {
                    if host.is_empty() || host.contains(['[', ']']) || port.contains(':') {
                        return Err(ProviderError::InvalidOrigin);
                    }
                    (host.to_ascii_lowercase(), Some(port))
                }
                None => {
                    if rest.contains(['[', ']']) {
                        return Err(ProviderError::InvalidOrigin);
                    }
                    (rest.to_ascii_lowercase(), None)
                }
            }
        };
        if host.is_empty() {
            return Err(ProviderError::InvalidOrigin);
        }
        let port = match port {
            Some(p) => {
                let p: u16 = p.parse().map_err(|_| ProviderError::InvalidOrigin)?;
                if p == 0 {
                    return Err(ProviderError::InvalidOrigin);
                }
                Some(p)
            }
            None => None,
        };
        if scheme == "http" && !matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1" | "[::1]")
        {
            return Err(ProviderError::InvalidOrigin);
        }
        let mut normalized = format!("{scheme}://{host}");
        if let Some(p) = port {
            normalized.push_str(&format!(":{p}"));
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Relative entry path bound to one protocol profile of a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndpointPath(String);

impl EndpointPath {
    pub fn new(path: &str) -> Result<Self, ProviderError> {
        if !path.starts_with('/')
            || path.len() > 256
            || path.contains(['?', '#', ' ', '\t'])
            || path.ends_with('/')
        {
            return Err(ProviderError::InvalidPath);
        }
        let mut segments = path.split('/').skip(1).peekable();
        while let Some(segment) = segments.next() {
            if segment.is_empty() || matches!(segment, "." | "..") {
                return Err(ProviderError::InvalidPath);
            }
            if segments.peek().is_none() && !segment.is_ascii() {
                return Err(ProviderError::InvalidPath);
            }
        }
        Ok(Self(path.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Trusted provider facts: origin, per-protocol relative entries and the auth
/// scheme for one credential kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDefinition {
    pub id: ProviderId,
    pub origin: TrustedOrigin,
    pub chat_completions: EndpointPath,
    pub responses: EndpointPath,
    pub auth: AuthScheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_origin_accepts_https_and_loopback_http_only() {
        for ok in [
            "https://api.deepseek.com",
            "https://api.xiaomimimo.com",
            "https://api.deepseek.com:8443",
            "http://127.0.0.1:39217",
            "http://localhost:39217",
            "http://[::1]:39217",
        ] {
            assert!(TrustedOrigin::parse(ok).is_ok(), "expected ok: {ok}");
        }
        for bad in [
            "http://api.deepseek.com",
            "ftp://api.deepseek.com",
            "https://user:pass@api.deepseek.com",
            "https://api.deepseek.com/path",
            "https://api.deepseek.com?x=1",
            "https://api.deepseek.com#frag",
            "https://",
            "https://api.deepseek.com:0",
            "api.deepseek.com",
        ] {
            assert!(TrustedOrigin::parse(bad).is_err(), "expected err: {bad}");
        }
    }

    #[test]
    fn trusted_origin_normalizes_case_and_keeps_port() {
        let origin = TrustedOrigin::parse("HTTPS://API.DeepSeek.COM").unwrap();
        assert_eq!(origin.as_str(), "https://api.deepseek.com");
        let origin = TrustedOrigin::parse("http://127.0.0.1:39217").unwrap();
        assert_eq!(origin.as_str(), "http://127.0.0.1:39217");
    }

    #[test]
    fn endpoint_paths_are_concrete_relative_entries() {
        for ok in ["/chat/completions", "/responses", "/v1/responses"] {
            assert!(EndpointPath::new(ok).is_ok(), "expected ok: {ok}");
        }
        for bad in [
            "chat/completions",
            "/",
            "/v1//responses",
            "/v1/../secret",
            "/responses?beta=1",
            "/responses#frag",
            "/responses/",
        ] {
            assert!(EndpointPath::new(bad).is_err(), "expected err: {bad}");
        }
    }

    #[test]
    fn provider_ids_are_bounded_identifiers() {
        assert_eq!(ProviderId::new("deepseek").unwrap().as_str(), "deepseek");
        assert!(ProviderId::new("Deep Seek").is_err());
        assert!(ProviderId::new("").is_err());
    }
}
