//! Complete canonical model facts for Qwen3.8 27B (`qwen/qwen3.8-27b`).
//!
//! Facts follow the OpenRouter model and endpoint records reverified on 2026-08-24. The canonical
//! context ceiling remains distinct from narrower Provider endpoint limits.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Qwen3.8 27B.
pub(crate) const ID: &str = "qwen/qwen3.8-27b";

/// Builds the complete model facts for Qwen3.8 27B.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.8 27B".to_owned(),
        description: Some(
            "Dense 27-billion-parameter vision-language model for coding, research, multimodal interaction, and long-running agents."
                .to_owned(),
        ),
        tokenizer: Some("Qwen".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_000_000),
                Some(1_000_000),
                Some(131_072),
            ),
            input_modalities: Some(vec![
                InputModality::Text,
                InputModality::Image,
                InputModality::Video,
            ]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "frequency_penalty",
                "include_reasoning",
                "logit_bias",
                "logprobs",
                "max_tokens",
                "presence_penalty",
                "repetition_penalty",
                "response_format",
                "seed",
                "stop",
                "structured_outputs",
                "temperature",
                "tool_choice",
                "tools",
                "top_k",
                "top_logprobs",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            reasoning: ReasoningProfile::supported([
                ReasoningLevel::XHigh,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]),
        }),
    }
}
