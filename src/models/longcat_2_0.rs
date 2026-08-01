//! LongCat 系列的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// 构造 LongCat-2.0 模型事实；具体 Provider endpoint 与上游 model id 不属于此定义。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: "meituan/longcat-2.0".to_owned(),
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
