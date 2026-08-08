//! Complete canonical model facts for Qwen3.8 Max (`qwen/qwen3.8-max`).
//!
//! The reference metadata is the dated OpenRouter snapshot `qwen/qwen3.8-max-20260803`; the
//! stable catalog ID remains the mainline alias.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Stable OpenBridge catalog ID for Qwen3.8 Max.
pub(crate) const ID: &str = "qwen/qwen3.8-max";

/// Builds the context, modality, parameter, and reasoning facts for Qwen3.8 Max.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.8 Max".to_owned(),
        description: Some(
            "Qwen3.8 flagship multimodal reasoning model for complex reasoning, visual understanding, coding, and agentic workloads."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_000_000), Some(1_000_000), Some(131_072)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::Video,
        ]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("Qwen".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logprobs",
            "max_tokens",
            "presence_penalty",
            "reasoning",
            "reasoning_effort",
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
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::Minimal,
        ],
    }
}
