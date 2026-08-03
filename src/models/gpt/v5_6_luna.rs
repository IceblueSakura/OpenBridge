//! Complete canonical model facts for GPT-5.6 Luna.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Stable OpenBridge catalog ID for GPT-5.6 Luna.
pub(crate) const ID: &str = "openai/gpt-5.6-luna";

/// Builds the GPT-5.6 Luna model facts confirmed by the LiteLLM configuration.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.6 Luna".to_owned(),
        description: Some(
            "Fast, cost-efficient GPT-5.6 model for chat, classification, and lightweight agents."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), Some(1_050_000), Some(128_000)),
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
