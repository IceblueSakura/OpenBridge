//! Xiaomi MiMo 系列的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// 返回 MiMo 系列所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![mimo_v2_5_pro(), mimo_v2_5()]
}

/// 构造 MiMo-V2.5-Pro 的 context、参数和 reasoning 事实。
fn mimo_v2_5_pro() -> ModelConfig {
    ModelConfig {
        id: "xiaomi/mimo-v2.5-pro".to_owned(),
        name: "MiMo-V2.5-Pro".to_owned(),
        description: Some(
            "Xiaomi flagship model for complex software engineering and long-horizon agentic tasks."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), Some(131_072)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
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
        reasoning_levels: Vec::new(),
    }
}

/// 构造 MiMo-V2.5 的 context、参数和 reasoning 事实。
fn mimo_v2_5() -> ModelConfig {
    ModelConfig {
        id: "xiaomi/mimo-v2.5".to_owned(),
        name: "MiMo-V2.5".to_owned(),
        description: Some(
            "Native omnimodal Xiaomi model for cost-efficient agents and image or video understanding."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_050_000), Some(131_072)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
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
        reasoning_levels: Vec::new(),
    }
}
