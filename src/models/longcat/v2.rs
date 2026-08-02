//! LongCat 2.x 版本线的完整 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// LongCat 2.0 在 OpenBridge 模型目录中的稳定 id。
pub(crate) const ID: &str = "meituan/longcat-2.0";

/// 构造 LongCat 2.0 的完整模型事实。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "LongCat 2.0".to_owned(),
        description: Some(
            "Sparse Mixture-of-Experts model for coding, repository changes, and long-horizon agents."
                .to_owned(),
        ),
        // 目录公布的是总 context window；当前路由只校验单独声明的输出上限，组合上限仍由上游裁决。
        context_length: ModelContextLength::new(Some(1_048_756), Some(262_144)),
        supported_parameters: vec![
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
            "repetition_penalty",
            "seed",
            "stop",
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
        reasoning_levels: Vec::new(),
    }
}
