//! GPT-5.3 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 构造人工修订的 GPT-5.3 Codex Spark 模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "openai/gpt-5.3-codex-spark".to_owned(),
        name: "GPT-5.3 Codex Spark".to_owned(),
        description: None,
        context_length: ModelContextLength::new(Some(128_000), Some(128_000)),
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
