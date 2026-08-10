//! Conservative canonical model facts for NVIDIA Nemotron 3 Embed 1B.
//!
//! Embedding facts follow the 2026-08-10 direct probe of the NVIDIA API Catalog
//! `/v1/embeddings` endpoint: `nemotron-3-embed-1b` returned two 2048-dimension
//! float vectors for a string-array input. Only that confirmed shape is recorded.

use crate::registry::{CanonicalModelTask, EmbeddingModelProfile, InputModality, ModelConfig};

/// Stable OpenBridge catalog ID for NVIDIA Nemotron 3 Embed 1B.
pub(crate) const ID: &str = "nvidia/nemotron-3-embed-1b";

/// Builds the provider-independent text-embedding facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "NVIDIA Nemotron 3 Embed 1B".to_owned(),
        description: Some(
            "NVIDIA Nemotron 3 Embed 1B text-embedding model for semantic retrieval and classification."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::Embedding(EmbeddingModelProfile {
            max_input_tokens: None,
            input_modalities: Some(vec![InputModality::Text]),
            supported_parameters: ["dimensions", "encoding_format"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}
