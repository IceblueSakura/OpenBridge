//! Complete canonical model facts for Qwen3.7 Max (`qwen/qwen3.7-max`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Qwen3.7 Max.
pub(crate) const ID: &str = "qwen/qwen3.7-max";

/// Builds the context, parameter, and reasoning facts for Qwen3.7 Max.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.7 Max".to_owned(),
        description: Some(
            "Qwen3.7 flagship model for agent-centric coding, office, and productivity workloads."
                .to_owned(),
        ),
        tokenizer: Some("Qwen".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_000_000),
                Some(1_000_000),
                Some(131_072),
            ),
            input_modalities: Some(vec![InputModality::Text]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "frequency_penalty",
                "include_reasoning",
                "logprobs",
                "max_tokens",
                "presence_penalty",
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
                ReasoningLevel::Max,
                ReasoningLevel::XHigh,
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::Minimal,
                ReasoningLevel::None,
            ]),
        }),
    }
}
