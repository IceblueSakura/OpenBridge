//! Complete canonical model facts for MiMo-V2.5 (`xiaomi/mimo-v2.5`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

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
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
        context_length: ModelContextLength::new(Some(1_050_000), Some(1_050_000), Some(131_072)),
        input_modalities: Some(vec![
            InputModality::Text,
            InputModality::Audio,
            InputModality::Image,
            InputModality::Video,
        ]),
        output_modalities: Some(vec![OutputModality::Text]),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
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
        reasoning: ReasoningProfile::supported([
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ]),
        }),
    }
}
