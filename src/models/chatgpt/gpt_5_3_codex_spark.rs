//! Complete canonical model facts for ChatGPT GPT-5.3 Codex Spark (`chatgpt/gpt-5.3-codex-spark`).

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, ModelConfig, ModelContextLength, ReasoningLevel,
    ReasoningProfile,
};

/// Stable OpenBridge catalog ID for the ChatGPT subscription profile.
pub(crate) const ID: &str = "chatgpt/gpt-5.3-codex-spark";

/// Builds manually curated model facts for the ChatGPT GPT-5.3 Codex Spark profile.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.3 Codex Spark".to_owned(),
        description: None,
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(Some(128_000), None, Some(128_000)),
            input_modalities: None,
            output_modalities: None,
            supported_parameters: Vec::new(),
            reasoning: ReasoningProfile::supported([
                ReasoningLevel::XHigh,
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
            ]),
        }),
    }
}
