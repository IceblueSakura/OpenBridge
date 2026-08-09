//! Conservative canonical model facts for Qwen Image 3.0 Pro.
//!
//! Image capabilities follow the qwen-image-3.0-pro model-market snapshot dated 2026-07-20.
//! The profile records only modalities confirmed by the model catalog.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningProfile,
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
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(None, None, None),
            input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
            output_modalities: Some(vec![OutputModality::Image]),
            supported_parameters: Vec::new(),
            reasoning: ReasoningProfile::Unknown,
        }),
    }
}
