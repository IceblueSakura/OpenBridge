//! Complete canonical model facts for Kimi K3 (`moonshotai/kimi-k3`).

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningLevel,
    ReasoningSupport,
};

/// Builds the complete model facts for Kimi K3.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "moonshotai/kimi-k3".to_owned(),
        name: "Kimi K3".to_owned(),
        description: Some(
            "Open-weight multimodal reasoning model for coding, knowledge work, and long-horizon agents."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), Some(1_048_576), None),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
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
        reasoning_levels: vec![
            ReasoningLevel::Max,
            ReasoningLevel::High,
            ReasoningLevel::Low,
        ],
    }
}
