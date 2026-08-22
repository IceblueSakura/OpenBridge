//! Conservative canonical model facts for Qwen Image 3.0 Pro.
//!
//! Image capabilities follow the qwen-image-3.0-pro model-market snapshot dated 2026-07-20.
//! The profile records only modalities confirmed by the model catalog.

use crate::registry::{
    CanonicalModelTask, ImageGenerationModelProfile, ModelConfig, ModelContextLength,
};

/// Stable OpenBridge catalog ID for Qwen Image 3.0 Pro.
pub(crate) const ID: &str = "qwen/qwen-image-3.0-pro";

/// Builds the provider-independent image-generation facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Image 3.0 Pro".to_owned(),
        description: Some(
            "Qwen Image 3.0 Pro model for image generation and editing with improved text rendering, realistic details, and semantic adherence."
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
