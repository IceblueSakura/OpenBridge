//! Complete canonical model facts for the LongCat 2.x line.

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

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
        context_length: ModelContextLength::new(Some(1_048_756), None, Some(262_144)),
        mode: None,
        input_modalities: None,
        output_modalities: None,
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
        reasoning_levels: Vec::new(),
    }
}
