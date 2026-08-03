//! DeepSeek V4 Pro 的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// DeepSeek V4 Pro 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const ID: &str = "deepseek/deepseek-v4-pro";

/// 构造 DeepSeek V4 Pro 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "DeepSeek V4 Pro".to_owned(),
        description: Some(
            "Large Mixture-of-Experts model for advanced reasoning, coding, and agent workflows."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), None, Some(384_000)),
        mode: None,
        input_modalities: None,
        output_modalities: None,
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
