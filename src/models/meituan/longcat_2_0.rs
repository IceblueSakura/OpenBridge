//! Complete canonical model facts for LongCat 2.0 (`meituan/longcat-2.0`).

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Stable OpenBridge catalog ID for LongCat 2.0.
pub(crate) const ID: &str = "meituan/longcat-2.0";

/// Builds the complete model facts for LongCat 2.0.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "LongCat 2.0".to_owned(),
        description: Some(
            "Sparse Mixture-of-Experts model for coding, repository changes, and long-horizon agents."
                .to_owned(),
        ),
        // The catalog publishes the total context window; routing validates only the declared output limit, while the upstream enforces the combined limit.
        context_length: ModelContextLength::new(
            Some(1_048_756),
            Some(1_048_756),
            Some(262_144),
        ),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: vec![
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
            "repetition_penalty",
            "seed",
            "stop",
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
        reasoning_levels: vec![ReasoningLevel::High],
    }
}
