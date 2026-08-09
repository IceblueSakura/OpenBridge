//! Complete canonical model facts for DeepSeek V4 Flash (`deepseek/deepseek-v4-flash`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for DeepSeek V4 Flash.
pub(crate) const ID: &str = "deepseek/deepseek-v4-flash";

/// Builds the context, parameter, and reasoning facts for DeepSeek V4 Flash.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "DeepSeek V4 Flash".to_owned(),
        description: Some(
            "Efficiency-optimized Mixture-of-Experts model for fast reasoning, coding, and agents."
                .to_owned(),
        ),
        tokenizer: Some("DeepSeek".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_048_576),
                Some(1_048_576),
                Some(393_216),
            ),
            input_modalities: Some(vec![InputModality::Text]),
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
                "top_a",
                "top_k",
                "top_logprobs",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            reasoning: ReasoningProfile::supported([
                ReasoningLevel::Max,
                ReasoningLevel::High,
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]),
        }),
    }
}
