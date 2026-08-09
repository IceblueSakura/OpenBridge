//! Complete canonical model facts for Qwen3.6 27B (`qwen/qwen3.6-27b`).
//!
//! The reference metadata is the dated `qwen3.6-27b-20260422` snapshot; the stable catalog ID
//! remains the mainline alias.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Qwen3.6 27B.
pub(crate) const ID: &str = "qwen/qwen3.6-27b";

/// Builds the context, modality, parameter, and reasoning facts for Qwen3.6 27B.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen3.6 27B".to_owned(),
        description: Some(
            "Dense 27-billion-parameter Qwen3.6 multimodal model for agentic coding, visual understanding, and reasoning."
                .to_owned(),
        ),
        tokenizer: Some("Qwen3".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
        context_length: ModelContextLength::new(Some(262_144), Some(262_144), Some(65_536)),
        input_modalities: Some(vec![
            InputModality::Text,
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
        reasoning: ReasoningProfile::supported([ReasoningLevel::High, ReasoningLevel::None]),
        }),
    }
}
