//! Complete canonical model facts for Xiaomi MiMo-V2.5-Pro.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
};

/// Stable OpenBridge catalog ID for MiMo-V2.5-Pro.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5-pro";

/// Builds the context, parameter, and reasoning facts for MiMo-V2.5-Pro.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5-Pro".to_owned(),
        description: Some(
            "Xiaomi flagship model for complex software engineering and long-horizon agentic tasks."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), Some(1_050_000), Some(131_072)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
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
