//! Complete canonical model facts for the GPT-5.3 line.

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// Builds manually curated model facts for GPT-5.3 Codex Spark.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "openai/gpt-5.3-codex-spark".to_owned(),
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
