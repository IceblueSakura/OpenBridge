//! Tencent Hy3 的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 构造 Hy3 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "tencent/hy3".to_owned(),
        name: "Hy3".to_owned(),
        description: Some(
            "Tencent Mixture-of-Experts model for configurable reasoning and production agent workflows."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(262_144), Some(128_000)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "max_completion_tokens",
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
            "top_p",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: vec![
            ReasoningLevel::High,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    }
}
