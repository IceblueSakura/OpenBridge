//! DeepSeek V4 Flash 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningLevel, ReasoningSupport};

/// 构造 DeepSeek V4 Flash 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "deepseek/deepseek-v4-flash",
        "DeepSeek V4 Flash",
        Some(1_048_576),
        Some(384_000),
        super::catalog::DEEPSEEK_FLASH_PARAMETERS,
        ReasoningSupport::Supported,
        &[ReasoningLevel::XHigh, ReasoningLevel::High],
    )
}
