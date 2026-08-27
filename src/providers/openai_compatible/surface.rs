//! Typed operation surface for one OpenAI-compatible Provider.

use crate::core::{
    ApiCapabilities, EmbeddingsCapabilities, ImagesGenerationsCapabilities,
    ProviderChatCompletionsCapabilities, ProviderOperationCapabilities,
    ProviderResponsesCapabilities,
};

/// One fixed OpenAI-compatible operation endpoint paired with its capability ceiling.
#[derive(Clone, Copy)]
pub(crate) struct OpenAiCompatibleEndpoint<T> {
    relative_path: &'static str,
    capabilities: T,
}

impl<T> OpenAiCompatibleEndpoint<T> {
    /// Pairs one trusted relative endpoint path with the capabilities implemented there.
    pub(crate) const fn new(relative_path: &'static str, capabilities: T) -> Self {
        Self {
            relative_path,
            capabilities,
        }
    }
}

/// Closed operation surface shared by one Provider contract and its wire adapter.
#[derive(Clone, Copy)]
pub(crate) struct OpenAiCompatibleApiSurface {
    chat_completions: Option<OpenAiCompatibleEndpoint<ProviderChatCompletionsCapabilities>>,
    responses: Option<OpenAiCompatibleEndpoint<ProviderResponsesCapabilities>>,
    embeddings: Option<OpenAiCompatibleEndpoint<EmbeddingsCapabilities>>,
    images: Option<OpenAiCompatibleEndpoint<ImagesGenerationsCapabilities>>,
}

impl OpenAiCompatibleApiSurface {
    /// Creates one operation surface; an absent endpoint is an unsupported operation.
    pub(crate) const fn new(
        chat_completions: Option<OpenAiCompatibleEndpoint<ProviderChatCompletionsCapabilities>>,
        responses: Option<OpenAiCompatibleEndpoint<ProviderResponsesCapabilities>>,
        embeddings: Option<OpenAiCompatibleEndpoint<EmbeddingsCapabilities>>,
    ) -> Self {
        Self {
            chat_completions,
            responses,
            embeddings,
            images: None,
        }
    }

    /// Attaches an Images Generations endpoint to an otherwise OpenAI-compatible surface.
    pub(crate) const fn with_images(
        mut self,
        images: Option<OpenAiCompatibleEndpoint<ImagesGenerationsCapabilities>>,
    ) -> Self {
        self.images = images;
        self
    }

    /// Projects the Provider capability contract from the same typed endpoint descriptors.
    pub(crate) const fn capabilities(&'static self) -> ApiCapabilities {
        ApiCapabilities::from_indexed_operations([
            match &self.chat_completions {
                Some(endpoint) => Some(ProviderOperationCapabilities::ChatCompletions(
                    &endpoint.capabilities,
                )),
                None => None,
            },
            match &self.responses {
                Some(endpoint) => Some(ProviderOperationCapabilities::Responses(
                    &endpoint.capabilities,
                )),
                None => None,
            },
            match &self.embeddings {
                Some(endpoint) => Some(ProviderOperationCapabilities::Embeddings(
                    &endpoint.capabilities,
                )),
                None => None,
            },
            match &self.images {
                Some(endpoint) => Some(ProviderOperationCapabilities::ImagesGenerations(
                    &endpoint.capabilities,
                )),
                None => None,
            },
        ])
    }

    /// Returns the trusted Chat Completions path when that operation is present.
    pub(super) const fn chat_path(self) -> Option<&'static str> {
        match self.chat_completions {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }

    /// Returns the trusted Responses path when that operation is present.
    pub(super) const fn responses_path(self) -> Option<&'static str> {
        match self.responses {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }

    /// Returns the trusted Embeddings path when that operation is present.
    pub(super) const fn embeddings_path(self) -> Option<&'static str> {
        match self.embeddings {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }

    /// Returns the trusted Images Generations path when that operation is present.
    pub(super) const fn images_path(self) -> Option<&'static str> {
        match self.images {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }
}
