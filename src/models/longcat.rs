//! LongCat 系列的 canonical 模型事实。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

/// LongCat-2.0 在 OpenBridge 模型目录中的稳定 id。
pub const MODEL_ID: &str = "longcat/longcat-2.0";

/// 构造 LongCat-2.0 模型事实；具体 Provider endpoint 与上游 model id 不属于此定义。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: MODEL_ID.to_owned(),
        name: "LongCat-2.0".to_owned(),
        description: Some(
            "LongCat-2.0 text model with a 1,048,756-token context window.".to_owned(),
        ),
        // 目录公布的是总 context window；当前路由只校验单独声明的输出上限，组合上限仍由上游裁决。
        context_length: ModelContextLength::new(Some(1_048_756), Some(262_144)),
        supported_parameters: vec![
            "frequency_penalty".to_owned(),
            "include_reasoning".to_owned(),
            "logit_bias".to_owned(),
            "max_tokens".to_owned(),
            "min_p".to_owned(),
            "presence_penalty".to_owned(),
            "reasoning".to_owned(),
            "repetition_penalty".to_owned(),
            "seed".to_owned(),
            "stop".to_owned(),
            "temperature".to_owned(),
            "tool_choice".to_owned(),
            "tools".to_owned(),
            "top_k".to_owned(),
            "top_p".to_owned(),
        ],
        reasoning: ReasoningSupport::Supported,
        reasoning_levels: Vec::new(),
    }
}
