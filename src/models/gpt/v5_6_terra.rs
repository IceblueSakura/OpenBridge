//! GPT-5.6 Terra 的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// GPT-5.6 Terra 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const ID: &str = "openai/gpt-5.6-terra";

/// 构造 LiteLLM 配置已确认的 GPT-5.6 Terra 模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GPT-5.6 Terra".to_owned(),
        description: Some(
            "Balanced GPT-5.6 model for everyday coding, reasoning, and agentic workflows."
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
            ReasoningLevel::Max,
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    }
}
