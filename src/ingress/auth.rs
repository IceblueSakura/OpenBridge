use std::fmt;

use http::{HeaderMap, header::AUTHORIZATION};
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

pub struct StaticBearerCredential {
    secret: SecretString,
}

impl StaticBearerCredential {
    pub fn new(secret: SecretString) -> Self {
        Self { secret }
    }

    pub fn authenticate(&self, headers: &HeaderMap) -> bool {
        let Some(candidate) = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
        else {
            return false;
        };
        let expected = self.secret.expose_secret().as_bytes();
        !expected.is_empty()
            && candidate.len() == expected.len()
            && bool::from(candidate.as_bytes().ct_eq(expected))
    }
}

impl fmt::Debug for StaticBearerCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticBearerCredential")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}
