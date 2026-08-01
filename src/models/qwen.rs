//! Qwen 系列的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// 返回 Qwen 系列所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![qwen3_7_max(), qwen3_7_plus()]
}

/// 构造 Qwen3.7 Max 的 context、参数和 reasoning 事实。
fn qwen3_7_max() -> ModelConfig {
    ModelConfig {
        id: "qwen/qwen3.7-max".to_owned(),
        name: "Qwen3.7 Max".to_owned(),
        description: Some(
            "Qwen3.7 flagship model for agent-centric coding, office, and productivity workloads."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_000_000), Some(131_072)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logprobs",
            "max_tokens",
            "presence_penalty",
            "reasoning",
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

/// 构造 Qwen3.7 Plus 的 context、参数和 reasoning 事实。
fn qwen3_7_plus() -> ModelConfig {
    ModelConfig {
        id: "qwen/qwen3.7-plus".to_owned(),
        name: "Qwen3.7 Plus".to_owned(),
        description: Some(
            "Cost-effective multimodal Qwen3.7 model for coding, tool use, productivity, and GUI agents."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_000_000), Some(131_072)),
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logprobs",
            "max_tokens",
            "presence_penalty",
            "reasoning",
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
