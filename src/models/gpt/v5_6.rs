//! GPT-5.6 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// GPT-5.6 Sol 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const SOL_ID: &str = "openai/gpt-5.6-sol";

/// 返回 GPT-5.6 版本线的全部模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![sol(), terra(), luna()]
}

/// 构造 LiteLLM 配置已确认的 GPT-5.6 Sol 模型事实。
fn sol() -> ModelConfig {
    ModelConfig {
        id: SOL_ID.to_owned(),
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

/// 构造 LiteLLM 配置已确认的 GPT-5.6 Terra 模型事实。
fn terra() -> ModelConfig {
    ModelConfig {
        id: "openai/gpt-5.6-terra".to_owned(),
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

/// 构造 LiteLLM 配置已确认的 GPT-5.6 Luna 模型事实。
fn luna() -> ModelConfig {
    ModelConfig {
        id: "openai/gpt-5.6-luna".to_owned(),
        name: "GPT-5.6 Luna".to_owned(),
        description: Some(
            "Fast, cost-efficient GPT-5.6 model for chat, classification, and lightweight agents."
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
