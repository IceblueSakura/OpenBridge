//! Complete canonical model facts for ChatGPT GPT-5.5 (`chatgpt/gpt-5.5`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for the ChatGPT subscription profile.
pub(crate) const ID: &str = "chatgpt/gpt-5.5";

/// Builds the ChatGPT GPT-5.5 profile with its subscription context limits.
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
        context_length: ModelContextLength::new(Some(272_000), Some(272_000), Some(128_000)),
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
            "parallel_tool_calls",
            "response_format",
            "seed",
            "service_tier",
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
