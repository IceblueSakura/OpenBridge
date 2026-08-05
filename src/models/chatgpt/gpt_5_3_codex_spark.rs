//! Complete canonical model facts for ChatGPT GPT-5.3 Codex Spark (`chatgpt/gpt-5.3-codex-spark`).

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// Stable OpenBridge catalog ID for the ChatGPT subscription profile.
pub(crate) const ID: &str = "chatgpt/gpt-5.3-codex-spark";

/// Builds manually curated model facts for the ChatGPT GPT-5.3 Codex Spark profile.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.3 Codex Spark".to_owned(),
        description: None,
        context_length: ModelContextLength::new(Some(128_000), None, Some(128_000)),
        mode: None,
        input_modalities: None,
        output_modalities: None,
        tokenizer: None,
        knowledge_cutoff: None,
        supported_parameters: vec!["reasoning".to_owned()],
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
        ],
    }
}
