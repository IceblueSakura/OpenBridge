//! MiMo-V2.5-Pro 的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// 构造 MiMo-V2.5-Pro 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
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
