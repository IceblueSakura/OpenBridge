use bytes::Bytes;
use http::{Method, Uri};
use thiserror::Error;

use crate::core::{CapabilitySet, Protocol, ValidatedRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    OpenAi,
}

impl ProviderKind {
    pub(crate) fn from_config(value: &str) -> Option<Self> {
        match value {
            "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }

    pub(crate) fn capabilities(self) -> CapabilitySet {
        match self {
            Self::OpenAi => CapabilitySet {
                chat: true,
                responses: true,
                streaming: true,
                function_tools: true,
                structured_output: true,
                previous_response_id: true,
                background: false,
                response_store: false,
            },
        }
    }

    pub(crate) fn accepts_endpoint_profile(self, profile: &str) -> bool {
        match self {
            Self::OpenAi => profile == "public-api",
        }
    }
}

#[derive(Debug, Error)]
pub enum ProviderFailure {
    #[error("request protocol is not supported by this provider adapter")]
    UnsupportedProtocol,
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
