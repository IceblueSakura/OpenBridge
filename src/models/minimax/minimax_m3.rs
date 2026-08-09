//! Complete canonical model facts for MiniMax M3 (`minimax/minimax-m3`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for MiniMax M3.
pub(crate) const ID: &str = "minimax/minimax-m3";

/// Builds the complete model facts for MiniMax M3.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiniMax M3".to_owned(),
        description: Some(
            "Multimodal foundation model for long-horizon agentic work, coding, and visual inputs."
                .to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_048_576),
                Some(1_048_576),
                Some(512_000),
            ),
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
