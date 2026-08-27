//! Ordinary and sensitive header assembly for OpenAI-compatible requests.

use http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use zeroize::Zeroizing;

use crate::{
    core::OperationKind,
    credential::CredentialType,
    provider::{AdapterError, ProviderKind, SafeHeaders, SensitiveHeaders},
};

use super::OpenAiCompatibleAdapter;

impl OpenAiCompatibleAdapter {
    /// Builds the base ordinary headers for an OpenAI-compatible JSON request.
    pub(crate) fn prepare_headers(self) -> Result<SafeHeaders, AdapterError> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }

    /// Applies the ordinary-header transform defined by the concrete Provider.
    pub(crate) fn apply_request_header_hook(
        self,
        downstream_headers: &HeaderMap,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        (self.request_header_hook)(downstream_headers, headers)
    }

    /// Applies the Provider policy bound to one trusted routed operation and model.
    pub(crate) fn apply_routed_request_header_hook(
        self,
        operation: OperationKind,
        upstream_model: &str,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        (self.routed_request_header_hook)(operation, upstream_model, headers)
    }

    /// Applies fixed Provider request headers after the downstream-header hook.
    pub(crate) fn apply_configured_request_headers(
        self,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        self.request_headers.apply_to(headers)
    }

    /// Builds a Bearer authentication header bound to the Provider identity.
    pub(crate) fn prepare_auth_headers(
        self,
        credential: &crate::credential::UpstreamCredential<'_>,
    ) -> Result<SensitiveHeaders, AdapterError> {
        // Verify credential ownership to prevent cross-Provider secret reuse.
        if credential.provider() != self.kind {
            return Err(AdapterError::CredentialProviderMismatch);
        }
        let CredentialType::Upstream(kind) = credential.metadata().credential_type() else {
            return Err(AdapterError::CredentialKindMismatch);
        };
        if !self.kind.contract().credential_kinds().contains(&kind) {
            return Err(AdapterError::CredentialKindMismatch);
        }

        // Assemble the sensitive Bearer header inside a zeroizing string.
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);

        // Bind ChatGPT subscription requests to the selected account and conditional FedRAMP edge.
        if self.kind == ProviderKind::ChatGpt {
            let account_id = credential
                .expose_chatgpt_account_id()
                .ok_or(AdapterError::IncompleteAuthenticationContext)?;
            headers.insert(
                HeaderName::from_static("chatgpt-account-id"),
                Zeroizing::new(account_id.to_owned()),
            );
            let is_fedramp_account = credential
                .is_fedramp_account()
                .ok_or(AdapterError::IncompleteAuthenticationContext)?;
            if is_fedramp_account {
                headers.insert(
                    HeaderName::from_static("x-openai-fedramp"),
                    Zeroizing::new("true".to_owned()),
                );
            }
        }
        Ok(headers)
    }
}

/// Keeps routed headers unchanged for Providers without operation/model-specific policy.
pub(super) fn preserve_routed_request_headers(
    _operation: OperationKind,
    _upstream_model: &str,
    _headers: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
