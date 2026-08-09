//! Complete canonical model facts for ChatGPT GPT-5.6 Sol (`chatgpt/gpt-5.6-sol`).

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Stable OpenBridge catalog ID for the ChatGPT subscription profile.
pub(crate) const ID: &str = "chatgpt/gpt-5.6-sol";

/// Builds the ChatGPT GPT-5.6 Sol profile with its subscription context limits.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.6 Sol".to_owned(),
        description: Some(
            "OpenAI flagship model for complex reasoning, coding, and multi-step agentic workflows."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(272_000), Some(272_000), Some(128_000)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::File,
        ]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("GPT".to_owned()),
        knowledge_cutoff: Some("2026-02-16".to_owned()),
        supported_parameters: [
            "include_reasoning",
            "max_completion_tokens",
            "max_tokens",
            "parallel_tool_calls",
            "reasoning",
            "reasoning_effort",
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
