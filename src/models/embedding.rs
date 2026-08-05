//! Canonical facts for OpenAI embedding models.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
};

/// Stable OpenBridge catalog ID for `text-embedding-3-small`.
pub(crate) const TEXT_EMBEDDING_3_SMALL_ID: &str = "openai/text-embedding-3-small";

/// Returns all OpenAI embedding model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![text_embedding_3_small()]
}

/// Builds the provider-independent facts confirmed for `text-embedding-3-small`.
fn text_embedding_3_small() -> ModelConfig {
    ModelConfig {
        id: TEXT_EMBEDDING_3_SMALL_ID.to_owned(),
        name: "Text Embedding 3 Small".to_owned(),
        description: Some(
            "OpenAI embedding model for mapping text and token inputs to numeric vectors."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(8_192), Some(8_192), None),
        mode: Some(ModelMode::Embedding),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Embedding]),
        tokenizer: None,
        knowledge_cutoff: None,
        supported_parameters: ["encoding_format", "user"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        reasoning: ReasoningSupport::Unsupported,
        reasoning_levels: Vec::new(),
    }
}
