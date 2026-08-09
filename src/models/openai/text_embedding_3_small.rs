//! Canonical facts for OpenAI's `text-embedding-3-small` model.

use crate::registry::{CanonicalModelTask, EmbeddingModelProfile, InputModality, ModelConfig};

/// Stable OpenBridge catalog ID for `text-embedding-3-small`.
pub(crate) const ID: &str = "openai/text-embedding-3-small";

/// Builds the provider-independent facts confirmed for `text-embedding-3-small`.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Text Embedding 3 Small".to_owned(),
        description: Some(
            "OpenAI embedding model for mapping text and token inputs to numeric vectors."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::Embedding(EmbeddingModelProfile {
            max_input_tokens: Some(8_192),
            input_modalities: Some(vec![InputModality::Text]),
            supported_parameters: ["encoding_format", "user"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}
