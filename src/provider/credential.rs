use std::{env, fmt};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use super::ProviderKind;

pub struct CredentialLease {
    provider: ProviderKind,
    binding_id: String,
    secret_version: String,
    secret: SecretString,
}

impl CredentialLease {
    pub fn new(
        provider: ProviderKind,
        binding_id: impl Into<String>,
        secret_version: impl Into<String>,
        secret: SecretString,
    ) -> Self {
        Self {
            provider,
            binding_id: binding_id.into(),
            secret_version: secret_version.into(),
            secret,
        }
    }

    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub fn secret_version(&self) -> &str {
        &self.secret_version
    }

    pub(super) fn expose_secret(&self) -> &str {
        self.secret.expose_secret()
    }
}

impl fmt::Debug for CredentialLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialLease")
            .field("provider", &self.provider)
            .field("binding_id", &self.binding_id)
            .field("secret_version", &self.secret_version)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum CredentialSourceError {
    #[error("upstream credential is unavailable")]
    Unavailable,
    #[error("static upstream credential does not match the configured binding")]
    BindingMismatch,
}

pub enum CredentialSource {
    Environment,
    Fixed {
        locator: String,
        secret: SecretString,
    },
}

impl CredentialSource {
    pub fn environment() -> Self {
        Self::Environment
    }

    pub fn fixed(locator: impl Into<String>, secret: SecretString) -> Self {
        Self::Fixed {
            locator: locator.into(),
            secret,
        }
    }

    pub fn resolve(
        &self,
        provider: ProviderKind,
        binding_id: &str,
        locator: &str,
    ) -> Result<CredentialLease, CredentialSourceError> {
        let (secret, version) = match self {
            Self::Environment => {
                let secret = env::var(locator).map_err(|_| CredentialSourceError::Unavailable)?;
                if secret.is_empty() {
                    return Err(CredentialSourceError::Unavailable);
                }
                (SecretString::from(secret), "environment")
            }
            Self::Fixed {
                locator: expected_locator,
                secret,
            } => {
                if locator != expected_locator {
                    return Err(CredentialSourceError::BindingMismatch);
                }
                (
                    SecretString::from(secret.expose_secret().to_owned()),
                    "fixed",
                )
            }
        };
        Ok(CredentialLease::new(provider, binding_id, version, secret))
    }
}

impl fmt::Debug for CredentialSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment => formatter.write_str("CredentialSource::Environment"),
            Self::Fixed { locator, .. } => formatter
                .debug_struct("CredentialSource::Fixed")
                .field("locator", locator)
                .field("secret", &"[REDACTED]")
                .finish(),
        }
    }
}
