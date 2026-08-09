//! Complete canonical model facts for OpenAI GPT-5.5 (`openai/gpt-5.5`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for OpenAI GPT-5.5.
pub(crate) const ID: &str = "openai/gpt-5.5";

/// Builds the GPT-5.5 model facts confirmed by the LiteLLM configuration.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.5".to_owned(),
        description: Some(
            "OpenAI frontier model for complex professional work with strong reasoning and reliability."
                .to_owned(),
        ),
        tokenizer: Some("GPT".to_owned()),
        knowledge_cutoff: Some("2025-12-01".to_owned()),
        task: CanonicalModelTask::Generation(GenerationModelProfile {
        context_length: ModelContextLength::new(Some(1_050_000), Some(1_050_000), Some(128_000)),
        input_modalities: Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::File,
        ]),
        output_modalities: Some(vec![OutputModality::Text]),
        supported_parameters: [
            "include_reasoning",
            "max_completion_tokens",
            "max_tokens",
            "response_format",
            "seed",
            "structured_outputs",
            "tool_choice",
            "tools",
        ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        reasoning: ReasoningProfile::supported([
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ]),
        }),
    }
}
