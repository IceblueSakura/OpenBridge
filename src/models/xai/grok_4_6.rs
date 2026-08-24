//! Complete canonical model facts for Grok 4.6 (`xai/grok-4.6`).
//!
//! Facts follow the OpenRouter model and endpoint records plus direct protocol probes reverified on
//! 2026-08-24. OpenRouter's exact upstream deployment ID is `x-ai/grok-4.6`.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Grok 4.6.
pub(crate) const ID: &str = "xai/grok-4.6";

/// Builds the complete model facts confirmed for Grok 4.6.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "xAI Grok 4.6".to_owned(),
        description: Some(
            "Frontier Grok model for coding, knowledge work, STEM, and visual understanding."
                .to_owned(),
        ),
        tokenizer: Some("Grok".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(Some(500_000), Some(500_000), None),
            input_modalities: Some(vec![
                InputModality::Text,
                InputModality::Image,
                InputModality::File,
            ]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "include_reasoning",
                "logprobs",
                "max_tokens",
                "response_format",
                "seed",
                "stream_options",
                "structured_outputs",
                "temperature",
                "tool_choice",
                "tools",
                "top_logprobs",
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
