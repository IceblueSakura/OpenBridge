//! GLM-5.2 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// 构造 GLM-5.2 的完整模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "z-ai/glm-5.2".to_owned(),
        name: "GLM-5.2".to_owned(),
        description: Some(
            "Large-scale reasoning model for long-horizon agents and project-level software engineering."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(1_048_576), None, Some(131_072)),
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
            "parallel_tool_calls",
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
