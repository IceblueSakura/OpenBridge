//! Qwen3.7 Plus 的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// Qwen3.7 Plus 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const ID: &str = "qwen/qwen3.7-plus";

/// 构造 Qwen3.7 Plus 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
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
