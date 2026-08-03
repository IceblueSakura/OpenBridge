//! Complete canonical model facts for the NVIDIA Nemotron 3 line.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Stable OpenBridge catalog ID for Nemotron 3 Ultra.
pub(crate) const ULTRA_ID: &str = "nvidia/nemotron-3-ultra-550b-a55b";

/// Builds the complete model facts for Nemotron 3 Ultra.
pub(crate) fn ultra() -> ModelConfig {
    ModelConfig {
        id: ULTRA_ID.to_owned(),
        name: "Nemotron 3 Ultra 550B A55B".to_owned(),
        description: Some(
            "Hybrid Transformer-Mamba Mixture-of-Experts model for reasoning and agent orchestration."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(512_288), Some(512_288), None),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
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
        reasoning_levels: vec![ReasoningLevel::High, ReasoningLevel::Medium],
    }
}
