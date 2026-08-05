//! Complete canonical model facts for Qwen3.7 Plus (`qwen/qwen3.7-plus`).

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
};

/// Stable OpenBridge catalog ID for Qwen3.7 Plus.
pub(crate) const ID: &str = "qwen/qwen3.7-plus";

/// Builds the context, parameter, and reasoning facts for Qwen3.7 Plus.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.7 Plus".to_owned(),
        description: Some(
            "Cost-effective multimodal Qwen3.7 model for coding, tool use, productivity, and GUI agents."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_000_000), Some(1_000_000), Some(131_072)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
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
        reasoning_levels: Vec::new(),
    }
}
