//! DeepSeek V4 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 返回 DeepSeek V4 版本线的全部模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![pro(), flash()]
}

/// 构造 DeepSeek V4 Pro 的 context、参数和 reasoning 事实。
fn pro() -> ModelConfig {
    ModelConfig {
        id: "deepseek/deepseek-v4-pro".to_owned(),
        name: "DeepSeek V4 Pro".to_owned(),
        description: Some(
            "Large Mixture-of-Experts model for advanced reasoning, coding, and agent workflows."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), Some(384_000)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
            "reasoning_effort",
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
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![ReasoningLevel::XHigh, ReasoningLevel::High],
    }
}

/// 构造 DeepSeek V4 Flash 的 context、参数和 reasoning 事实。
fn flash() -> ModelConfig {
    ModelConfig {
        id: "deepseek/deepseek-v4-flash".to_owned(),
        name: "DeepSeek V4 Flash".to_owned(),
        description: Some(
            "Efficiency-optimized Mixture-of-Experts model for fast reasoning, coding, and agents."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), Some(393_216)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
            "reasoning_effort",
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
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![ReasoningLevel::XHigh, ReasoningLevel::High],
    }
}
