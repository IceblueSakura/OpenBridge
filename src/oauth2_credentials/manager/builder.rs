//! Startup loading and binding of managed OAuth2 credential files.

use std::{fmt, path::PathBuf, sync::Arc};

use crate::provider::ProviderKind;

use super::super::{
    document::parse_auth_document,
    storage::{read_auth_document, version_for_document},
};
use super::credential::ManagedOAuth2Credential;
use super::{OAuth2CredentialManager, OAuth2CredentialManagerError};

/// Startup builder that validates complete files while preserving expired refreshable bundles.
#[derive(Default)]
pub(crate) struct OAuth2CredentialManagerBuilder {
    credentials: Vec<Arc<ManagedOAuth2Credential>>,
}

impl OAuth2CredentialManagerBuilder {
    /// Creates an empty startup builder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Loads one Provider-bound auth file and retains its complete refreshable token bundle.
    pub(crate) fn load_auth_json_file(
        &mut self,
        provider: ProviderKind,
        pool_id: &str,
        path: PathBuf,
    ) -> Result<(), OAuth2CredentialManagerError> {
        // Reject duplicate Provider ownership before reading any locator.
        if self
            .credentials
            .iter()
            .any(|credential| credential.provider == provider)
        {
            return Err(OAuth2CredentialManagerError::DuplicateProvider);
        }

        // Read and validate complete document shape while allowing an expired access token to refresh.
        let document = read_auth_document(&path).map_err(|_| OAuth2CredentialManagerError::Read)?;
        let bundle = parse_auth_document(provider, &document, false)?;
        let version = version_for_document(&document);

        // Bind the source and mutable lifecycle state to the compile-time Provider identity.
        self.credentials.push(Arc::new(ManagedOAuth2Credential::new(
            provider, pool_id, path, bundle, version,
        )));
        Ok(())
    }

    /// Freezes configured identities while retaining guarded internal lifecycle mutation.
    pub(crate) fn build(self) -> OAuth2CredentialManager {
        OAuth2CredentialManager {
            credentials: self.credentials,
        }
    }
}

impl fmt::Debug for OAuth2CredentialManagerBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2CredentialManagerBuilder")
            .field("configured_providers", &self.credentials.len())
            .finish()
    }
}
