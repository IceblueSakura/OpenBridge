use std::fmt;

use secrecy::{ExposeSecret, SecretString};

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
