//! DeepSeek V4 Pro 的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 构造 DeepSeek V4 Pro 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
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
