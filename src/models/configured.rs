//! 尚未绑定真实模型事实的 OpenAI-compatible 占位模型。

use crate::registry::{ModelConfig, ModelContextLength, ReasoningSupport};

pub(crate) const MODEL_ID: &str = "openai/configured-model";

/// 构造等待真实模型事实替换的占位配置。
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: MODEL_ID.to_owned(),
        name: "Configured OpenAI-compatible model".to_owned(),
        description: Some(
            "Replace this placeholder with metadata verified for the real upstream model."
                .to_owned(),
        ),
        context_length: ModelContextLength::default(),
        supported_parameters: Vec::new(),
        reasoning: ReasoningSupport::Unknown,
        reasoning_levels: Vec::new(),
    }
}
