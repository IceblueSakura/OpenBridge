//! GPT-5.6 Luna 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningSupport};

/// 构造 LiteLLM 配置已确认的 GPT-5.6 Luna 模型事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "openai/gpt-5.6-luna",
        "GPT-5.6 Luna",
        Some(272_000),
        None,
        &[],
        ReasoningSupport::Unknown,
        &[],
    )
}
