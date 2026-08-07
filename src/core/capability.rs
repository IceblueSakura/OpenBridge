//! Provider-independent capability ceilings grouped by operation family.
//!
//! Generation and Embeddings capabilities live in private submodules because their wire fields,
//! validation, and subset rules are independent. This facade preserves one provider-independent
//! API and combines the domains only in [`ApiCapabilities`].

mod embeddings;
mod generation;

pub use embeddings::{
    EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
};
pub(crate) use generation::GenerationCapabilities;
pub use generation::{
    ChatCompletionsCapabilities, HostedToolKind, ImageDetail, ImageInputCapabilities,
    ImageInputSource, ImageMediaType, ReasoningOutput, ResponseInclude, ResponsesCapabilities,
};

/// Protocol-specific capability ceilings for a Provider contract.
///
/// An Upstream API may disable capabilities supported by the Provider contract but cannot enable
/// unimplemented capabilities. The request path uses a separately precompiled Public Model
/// contract. Chat Completions, Responses, and Embeddings remain separate so observations from one
/// operation are not incorrectly applied to another.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApiCapabilities {
    /// Capability ceiling for the Chat Completions endpoint.
    pub chat_completions: ChatCompletionsCapabilities,
    /// Capability ceiling for the Responses endpoint.
    pub responses: ResponsesCapabilities,
    /// Capability ceiling for the Embeddings Create operation.
    pub embeddings: EmbeddingsCapabilities,
}
