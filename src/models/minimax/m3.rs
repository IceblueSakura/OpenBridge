//! MiniMax M3 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// 构造 MiniMax M3 的完整模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "minimax/minimax-m3".to_owned(),
        name: "MiniMax M3".to_owned(),
        description: Some(
            "Multimodal foundation model for long-horizon agentic work, coding, and visual inputs."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), None, Some(512_000)),
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
