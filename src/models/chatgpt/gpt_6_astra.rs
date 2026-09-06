//! Complete canonical model facts for ChatGPT GPT-6 Astra (`chatgpt/gpt-6-astra`).
//!
//! Capability, context, parameter, and reasoning facts mirror the verified `chatgpt/gpt-5.6-sol`
//! subscription profile because the upstream Models endpoint exposes Astra on the same Codex
//! backend without separate published limits; `knowledge_cutoff` stays `None` until an official
//! source states it.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for the ChatGPT subscription profile.
pub(crate) const ID: &str = "chatgpt/gpt-6-astra";

/// Builds the ChatGPT GPT-6 Astra profile with its subscription context limits.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-6 Astra".to_owned(),
        description: Some(
            "OpenAI GPT-6 model for complex reasoning, coding, and multi-step agentic workflows."
                .to_owned(),
        ),
        tokenizer: Some("GPT".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_000_000),
                Some(1_000_000),
                Some(128_000),
            ),
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
                ReasoningLevel::Max,
                ReasoningLevel::XHigh,
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]),
        }),
    }
}
