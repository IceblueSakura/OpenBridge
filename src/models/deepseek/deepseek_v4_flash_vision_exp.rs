//! Complete canonical model facts for DeepSeek V4 Flash Vision Exp
//! (`deepseek/deepseek-v4-flash-vision-exp`).
//!
//! Facts follow the OpenRouter model and endpoint records reverified on 2026-08-24.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for DeepSeek V4 Flash Vision Exp.
pub(crate) const ID: &str = "deepseek/deepseek-v4-flash-vision-exp";

/// Builds the complete model facts for DeepSeek V4 Flash Vision Exp.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "DeepSeek V4 Flash Vision Exp".to_owned(),
        description: Some(
            "Experimental vision-enabled DeepSeek V4 Flash model for image understanding, reasoning, agents, and coding."
                .to_owned(),
        ),
        tokenizer: Some("DeepSeek".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_048_576),
                Some(1_048_576),
                Some(384_000),
            ),
            input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "frequency_penalty",
                "include_reasoning",
                "logprobs",
                "max_tokens",
                "presence_penalty",
                "response_format",
                "stop",
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
                ReasoningLevel::Max,
                ReasoningLevel::High,
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]),
        }),
    }
}
