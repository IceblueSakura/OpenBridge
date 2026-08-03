//! Complete canonical model facts for the GPT-5.5 line.

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// Builds the GPT-5.5 model facts confirmed by the LiteLLM configuration.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "openai/gpt-5.5".to_owned(),
        name: "GPT-5.5".to_owned(),
        description: Some(
            "OpenAI frontier model for complex professional work with strong reasoning and reliability."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), None, Some(128_000)),
        mode: None,
        input_modalities: None,
        output_modalities: None,
        supported_parameters: [
            "include_reasoning",
            "max_completion_tokens",
            "max_tokens",
            "reasoning",
            "reasoning_effort",
            "response_format",
            "seed",
            "structured_outputs",
            "tool_choice",
            "tools",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    }
}
