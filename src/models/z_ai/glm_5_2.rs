//! Complete canonical model facts for GLM-5.2 (`z-ai/glm-5.2`).

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Stable OpenBridge catalog ID for GLM-5.2.
pub(crate) const ID: &str = "z-ai/glm-5.2";

/// Builds the complete model facts for GLM-5.2.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GLM-5.2".to_owned(),
        description: Some(
            "Large-scale reasoning model for long-horizon agents and project-level software engineering."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), Some(1_048_576), Some(131_072)),
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
            "parallel_tool_calls",
            "presence_penalty",
            "reasoning",
            "reasoning_effort",
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
        reasoning_levels: vec![ReasoningLevel::XHigh, ReasoningLevel::High],
    }
}
