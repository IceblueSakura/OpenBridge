//! GPT-5.6 Sol 的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// GPT-5.6 Sol 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const MODEL_ID: &str = "openai/gpt-5.6-sol";

/// 构造 LiteLLM 配置已确认的 GPT-5.6 Sol 模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: MODEL_ID.to_owned(),
        name: "GPT-5.6 Sol".to_owned(),
        description: Some(
            "OpenAI flagship model for complex reasoning, coding, and multi-step agentic workflows."
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
