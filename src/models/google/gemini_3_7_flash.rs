//! Complete canonical model facts for Gemini 3.7 Flash (`google/gemini-3.7-flash`).
//!
//! Facts follow the OpenRouter model and endpoint records plus direct protocol probes reverified on
//! 2026-08-24.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Gemini 3.7 Flash.
pub(crate) const ID: &str = "google/gemini-3.7-flash";

/// Builds the complete model facts confirmed for Gemini 3.7 Flash.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Google Gemini 3.7 Flash".to_owned(),
        description: Some(
            "Fast multimodal Gemini model for agentic workflows, coding, and multi-step reasoning."
                .to_owned(),
        ),
        tokenizer: Some("Gemini".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(Some(1_048_576), Some(1_048_576), Some(65_536)),
            input_modalities: Some(vec![
                InputModality::Text,
                InputModality::Image,
                InputModality::File,
                InputModality::Audio,
                InputModality::Video,
            ]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "include_reasoning",
                "max_tokens",
                "response_format",
                "seed",
                "stop",
                "stream_options",
                "structured_outputs",
                "temperature",
                "tool_choice",
                "tools",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            reasoning: ReasoningProfile::supported([
                ReasoningLevel::XHigh,
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
            ]),
        }),
    }
}
