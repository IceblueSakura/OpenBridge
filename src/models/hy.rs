//! Tencent HY 系列的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 返回 HY 系列所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![ModelConfig {
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
    }]
}
