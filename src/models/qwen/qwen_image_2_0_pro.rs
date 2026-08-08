//! Conservative canonical model facts for Qwen Image 2.0 Pro (`qwen/qwen-image-2.0-pro`).
//!
//! Image capabilities follow the `qwen-image-2.0-pro-2026-06-22` checkpoint.
//!
//! The current catalog records the confirmed image modalities while leaving token limits,
//! tokenizer, reasoning, and generic OpenAI-compatible parameters unknown because the model's
//! native image-generation API does not publish those facts in the shared model metadata shape.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
};

/// Stable OpenBridge catalog ID for Qwen Image 2.0 Pro.
pub(crate) const ID: &str = "qwen/qwen-image-2.0-pro";

/// Builds the provider-independent image-generation facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Image 2.0 Pro".to_owned(),
        description: Some(
            "Qwen-Image-2.0 Pro model for image generation and editing with enhanced text rendering, realistic textures, and semantic adherence."
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
