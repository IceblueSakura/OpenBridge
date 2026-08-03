//! Complete canonical model facts for Qwen3.7 Plus.

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

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
        context_length: ModelContextLength::new(Some(1_000_000), None, Some(131_072)),
        mode: None,
        input_modalities: None,
        output_modalities: None,
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
