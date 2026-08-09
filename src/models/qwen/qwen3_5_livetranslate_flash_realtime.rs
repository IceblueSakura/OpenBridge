//! Conservative canonical model facts for Qwen3.5 LiveTranslate Flash Realtime.
//!
//! Published limits and modalities follow the `qwen3.5-livetranslate-flash-realtime-2026-05-19`
//! checkpoint.
//!
//! The model is exposed by a native real-time translation API. This profile records the published
//! context and modality facts but does not invent Chat Completions parameter support.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Qwen3.5 LiveTranslate Flash Realtime.
pub(crate) const ID: &str = "qwen/qwen3.5-livetranslate-flash-realtime";

/// Builds the provider-independent real-time translation facts for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.5 LiveTranslate Flash Realtime".to_owned(),
        description: Some(
            "Qwen3.5 real-time audio and video translation model with visual enhancement and text/audio output."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(Some(53_248), Some(49_152), Some(4_096)),
            input_modalities: Some(vec![InputModality::Audio, InputModality::Image]),
            output_modalities: Some(vec![OutputModality::Text, OutputModality::Audio]),
            supported_parameters: Vec::new(),
            reasoning: ReasoningProfile::Unsupported,
        }),
    }
}
