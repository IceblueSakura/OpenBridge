//! Complete canonical model facts for the Tencent HY3 line.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Builds the complete model facts for HY3.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "tencent/hy3".to_owned(),
        name: "Hy3".to_owned(),
        description: Some(
            "Tencent Mixture-of-Experts model for configurable reasoning and production agent workflows."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(262_144), Some(262_144), Some(128_000)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "max_completion_tokens",
            "max_tokens",
            "min_p",
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
            "top_p",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![
            ReasoningLevel::High,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    }
}
