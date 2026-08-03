//! Complete canonical model facts for GPT-5.6 Sol.

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// Stable OpenBridge catalog ID for GPT-5.6 Sol.
pub(crate) const ID: &str = "openai/gpt-5.6-sol";

/// Builds the GPT-5.6 Sol model facts confirmed by the LiteLLM configuration.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.6 Sol".to_owned(),
        description: Some(
            "OpenAI flagship model for complex reasoning, coding, and multi-step agentic workflows."
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
            ReasoningLevel::Max,
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    }
}
