//! GPT-5.5 的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 构造 LiteLLM 配置已确认的 GPT-5.5 模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "openai/gpt-5.5".to_owned(),
        name: "GPT-5.5".to_owned(),
        description: Some(
            "OpenAI frontier model for complex professional work with strong reasoning and reliability."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), Some(128_000)),
        supported_parameters: [
            "include_reasoning",
            "max_completion_tokens",
            "max_tokens",
            "reasoning",
            "reasoning_effort",
            "response_format",
            "seed",
            "structured_outputs",
            "tool_choice",
            "tools",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    }
}
