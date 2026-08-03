//! Complete canonical model facts for Xiaomi MiMo-V2.5.

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// Stable OpenBridge catalog ID for MiMo-V2.5.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5";

/// Builds the context, parameter, and reasoning facts for MiMo-V2.5.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5".to_owned(),
        description: Some(
            "Native omnimodal Xiaomi model for cost-efficient agents and image or video understanding."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), None, Some(131_072)),
        mode: None,
        input_modalities: None,
        output_modalities: None,
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
            "repetition_penalty",
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
