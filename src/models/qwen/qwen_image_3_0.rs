//! Conservative canonical model facts for Qwen Image 3.0.
//!
//! Image capabilities follow the qwen-image-3.0 model-market snapshot dated 2026-08-04.
//! The profile records only modalities confirmed by the model catalog.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
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
        context_length: ModelContextLength::new(None, None, None),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
        output_modalities: Some(vec![OutputModality::Image]),
        tokenizer: None,
        knowledge_cutoff: None,
        supported_parameters: Vec::new(),
        reasoning: ReasoningSupport::Unknown,
        reasoning_levels: Vec::new(),
    }
}
