//! Complete canonical model facts for GLM-5.3 (`z-ai/glm-5.3`).
//!
//! Facts follow the OpenRouter model and endpoint records reverified on 2026-08-24.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for GLM-5.3.
pub(crate) const ID: &str = "z-ai/glm-5.3";

/// Builds the context, modality, parameter, and reasoning facts for GLM-5.3.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GLM-5.3".to_owned(),
        description: Some(
            "Large-scale reasoning model for complex software engineering and long-horizon agent tasks."
                .to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_048_576),
                Some(1_048_576),
                Some(131_072),
            ),
            input_modalities: Some(vec![InputModality::Text]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "include_reasoning",
                "max_tokens",
                "response_format",
                "temperature",
                "tool_choice",
                "tools",
                "top_k",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            reasoning: ReasoningProfile::supported([
                ReasoningLevel::Max,
                ReasoningLevel::High,
                ReasoningLevel::Low,
            ]),
        }),
    }
}
