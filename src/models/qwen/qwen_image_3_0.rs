//! Conservative canonical model facts for Qwen Image 3.0.
//!
//! Image capabilities follow the qwen-image-3.0 model-market snapshot dated 2026-08-04.
//! The profile records only modalities confirmed by the model catalog.

use crate::registry::{
    CanonicalModelTask, ImageGenerationModelProfile, ModelConfig, ModelContextLength,
};

/// Stable OpenBridge catalog ID for Qwen Image 3.0.
pub(crate) const ID: &str = "qwen/qwen-image-3.0";

/// Builds the provider-independent image-generation facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Image 3.0".to_owned(),
        description: Some(
            "Qwen Image 3.0 model for image generation with improved visual quality, text rendering, and semantic adherence."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::ImageGeneration(ImageGenerationModelProfile {
            context_length: ModelContextLength::new(None, None, None),
            supported_parameters: Vec::new(),
        }),
    }
}
