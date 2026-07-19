mod contracts;
mod credential;

pub use contracts::{
    AuthAdapter, CapabilityAdapter, ClassifiedProviderError, DecodedEvent, ErrorAdapter,
    EventDisposition, HeaderAdapter, ProviderErrorClass, ResponseAdapter, RetryHint, SafeHeaders,
    SensitiveHeaders,
};
pub use credential::{CredentialLease, CredentialSource, CredentialSourceError};

use bytes::Bytes;
use http::{
    HeaderValue, Method, StatusCode, Uri,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    core::{CapabilitySet, Protocol, ValidatedRequest},
    transport::sse::SseEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    OpenAi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    ApiKey,
}

impl CredentialKind {
    pub(crate) fn from_config(value: &str) -> Option<Self> {
        match value {
            "api_key" => Some(Self::ApiKey),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ProviderDescriptor {
    kind: ProviderKind,
    capabilities: CapabilitySet,
    endpoint_profiles: &'static [&'static str],
    credential_kinds: &'static [CredentialKind],
}

impl ProviderDescriptor {
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    pub fn endpoint_profiles(&self) -> &'static [&'static str] {
        self.endpoint_profiles
    }

    pub fn credential_kinds(&self) -> &'static [CredentialKind] {
        self.credential_kinds
    }
}

static OPENAI_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: ProviderKind::OpenAi,
    capabilities: CapabilitySet {
        chat: true,
        responses: true,
        streaming: true,
        function_tools: true,
        structured_output: true,
        previous_response_id: true,
        background: false,
        response_store: false,
    },
    endpoint_profiles: &["public-api"],
    credential_kinds: &[CredentialKind::ApiKey],
};

impl ProviderKind {
    pub(crate) fn from_config(value: &str) -> Option<Self> {
        match value {
            "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }

    pub fn descriptor(self) -> &'static ProviderDescriptor {
        match self {
            Self::OpenAi => &OPENAI_DESCRIPTOR,
        }
    }

    pub(crate) fn capabilities(self) -> CapabilitySet {
        *self.descriptor().capabilities()
    }

    pub(crate) fn accepts_endpoint_profile(self, profile: &str) -> bool {
        self.descriptor().endpoint_profiles().contains(&profile)
    }

    pub(crate) fn accepts_credential_kind(self, credential: CredentialKind) -> bool {
        self.descriptor().credential_kinds().contains(&credential)
    }
}

#[derive(Debug, Error)]
pub enum ProviderFailure {
    #[error("request protocol is not supported by this provider adapter")]
    UnsupportedProtocol,
    #[error("credential lease identity does not match the provider adapter")]
    CredentialProviderMismatch,
    #[error("sensitive header cannot be emitted by HeaderAdapter")]
    SensitiveHeaderInSafeSet,
    #[error("requested capabilities are not supported by the provider adapter")]
    UnsupportedCapabilities,
    #[error("provider authentication material cannot be encoded as an HTTP header")]
    InvalidAuthenticationHeader,
}

pub struct UpstreamRequestParts {
    method: Method,
    relative_uri: Uri,
    body: Bytes,
}

impl UpstreamRequestParts {
    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn relative_uri(&self) -> &Uri {
        &self.relative_uri
    }

    pub fn body(&self) -> &Bytes {
        &self.body
    }
}

pub trait RequestAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
    ) -> Result<UpstreamRequestParts, ProviderFailure>;
}

pub enum ProviderAdapter {
    OpenAi(OpenAiAdapter),
}

impl ProviderAdapter {
    pub fn for_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::OpenAi => Self::OpenAi(OpenAiAdapter),
        }
    }

    pub fn descriptor(&self) -> &'static ProviderDescriptor {
        match self {
            Self::OpenAi(_) => ProviderKind::OpenAi.descriptor(),
        }
    }

    pub(crate) fn build_outbound_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<http::HeaderMap, ProviderFailure> {
        let mut headers = self.build_headers()?.into_inner();
        self.build_auth_headers(credential)?
            .append_to(&mut headers)?;
        Ok(headers)
    }
}

impl RequestAdapter for ProviderAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
    ) -> Result<UpstreamRequestParts, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.encode_request(request),
        }
    }
}

pub struct OpenAiAdapter;

impl RequestAdapter for OpenAiAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
    ) -> Result<UpstreamRequestParts, ProviderFailure> {
        let relative_uri = match request.protocol() {
            Protocol::ChatCompletions => Uri::from_static("/v1/chat/completions"),
            Protocol::Responses => Uri::from_static("/v1/responses"),
        };

        Ok(UpstreamRequestParts {
            method: Method::POST,
            relative_uri,
            body: request.body().clone(),
        })
    }
}

impl HeaderAdapter for OpenAiAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }
}

impl AuthAdapter for OpenAiAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure> {
        if credential.provider() != ProviderKind::OpenAi {
            return Err(ProviderFailure::CredentialProviderMismatch);
        }
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);
        Ok(headers)
    }
}

impl ResponseAdapter for OpenAiAdapter {
    fn decode_event(
        &self,
        protocol: Protocol,
        event: SseEvent,
    ) -> Result<DecodedEvent, ProviderFailure> {
        let disposition = match protocol {
            Protocol::ChatCompletions if event.data() == "[DONE]" => EventDisposition::Completed,
            Protocol::Responses if event.event() == Some("response.completed") => {
                EventDisposition::Completed
            }
            Protocol::Responses
                if matches!(
                    event.event(),
                    Some("response.failed" | "response.incomplete")
                ) =>
            {
                EventDisposition::Failed
            }
            _ => EventDisposition::Continue,
        };
        Ok(DecodedEvent::new(event, disposition))
    }
}

impl ErrorAdapter for OpenAiAdapter {
    fn classify_status(&self, status: StatusCode) -> ClassifiedProviderError {
        let (class, retry_hint) = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                (ProviderErrorClass::InvalidRequest, RetryHint::Never)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                (ProviderErrorClass::Authentication, RetryHint::Never)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                (ProviderErrorClass::RateLimited, RetryHint::BeforeFirstEvent)
            }
            status if status.is_server_error() => (
                ProviderErrorClass::UpstreamUnavailable,
                RetryHint::BeforeFirstEvent,
            ),
            _ => (ProviderErrorClass::UpstreamFailure, RetryHint::Never),
        };
        ClassifiedProviderError::new(class, retry_hint)
    }
}

impl CapabilityAdapter for OpenAiAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure> {
        if requested.is_subset_of(*ProviderKind::OpenAi.descriptor().capabilities()) {
            Ok(())
        } else {
            Err(ProviderFailure::UnsupportedCapabilities)
        }
    }
}

impl HeaderAdapter for ProviderAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.build_headers(),
        }
    }
}

impl AuthAdapter for ProviderAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.build_auth_headers(credential),
        }
    }
}

impl ResponseAdapter for ProviderAdapter {
    fn decode_event(
        &self,
        protocol: Protocol,
        event: SseEvent,
    ) -> Result<DecodedEvent, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.decode_event(protocol, event),
        }
    }
}

impl ErrorAdapter for ProviderAdapter {
    fn classify_status(&self, status: StatusCode) -> ClassifiedProviderError {
        match self {
            Self::OpenAi(adapter) => adapter.classify_status(status),
        }
    }
}

impl CapabilityAdapter for ProviderAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.validate_capabilities(requested),
        }
    }
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::*;

    #[test]
    fn safe_header_debug_output_omits_values() {
        let mut headers = SafeHeaders::default();
        headers
            .insert(
                http::HeaderName::from_static("x-provider-metadata"),
                HeaderValue::from_static("metadata-test-value"),
            )
            .unwrap();

        assert!(!format!("{headers:?}").contains("metadata-test-value"));
    }

    #[test]
    fn safe_headers_reject_authentication_material() {
        let mut headers = SafeHeaders::default();

        let error = headers
            .insert(AUTHORIZATION, HeaderValue::from_static("forbidden"))
            .unwrap_err();

        assert!(matches!(error, ProviderFailure::SensitiveHeaderInSafeSet));
    }

    #[test]
    fn openai_auth_adapter_builds_the_expected_bearer_value_inside_the_crate_boundary() {
        let adapter = OpenAiAdapter;
        let lease = CredentialLease::new(
            ProviderKind::OpenAi,
            "binding",
            "version",
            SecretString::from("credential-test-value".to_owned()),
        );

        let headers = adapter.build_auth_headers(&lease).unwrap();

        assert_eq!(
            headers.expose(AUTHORIZATION),
            Some("Bearer credential-test-value")
        );
    }
}
