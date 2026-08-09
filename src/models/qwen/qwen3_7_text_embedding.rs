//! Conservative canonical model facts for Qwen3.7 Text Embedding.
//!
//! Embedding capabilities follow the qwen3.7-text-embedding model-market snapshot dated
//! 2026-07-15 and the Beijing OpenAI-compatible API limits. The profile records only the
//! confirmed text input, dimension, encoding, and request parameter facts.

use crate::registry::{CanonicalModelTask, EmbeddingModelProfile, InputModality, ModelConfig};

/// Stable OpenBridge catalog ID for Qwen3.7 Text Embedding.
pub(crate) const ID: &str = "qwen/qwen3.7-text-embedding";

/// Builds the provider-independent text-embedding facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.7 Text Embedding".to_owned(),
        description: Some(
            "Multilingual Qwen3.7 text-embedding model for semantic retrieval, clustering, and classification."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::Embedding(EmbeddingModelProfile {
            max_input_tokens: Some(128_000),
            input_modalities: Some(vec![InputModality::Text]),
            supported_parameters: ["dimensions", "encoding_format"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}
