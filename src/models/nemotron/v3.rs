//! NVIDIA Nemotron 3 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningLevel, ReasoningSupport};

/// Nemotron 3 Ultra 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const ULTRA_ID: &str = "nvidia/nemotron-3-ultra-550b-a55b";

/// 构造 Nemotron 3 Ultra 的完整模型事实。
pub(crate) fn ultra() -> ModelConfig {
    ModelConfig {
        id: ULTRA_ID.to_owned(),
        name: "Nemotron 3 Ultra 550B A55B".to_owned(),
        description: Some(
            "Hybrid Transformer-Mamba Mixture-of-Experts model for reasoning and agent orchestration."
                .to_owned(),
        ),
        context_length: ModelContextLength::new(Some(512_288), None),
        mode: None,
        input_modalities: None,
        output_modalities: None,
        supported_parameters: [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
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
        reasoning_levels: vec![ReasoningLevel::High, ReasoningLevel::Medium],
    }
}
