//! Qwen3.7 Max 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningSupport};

/// 构造 Qwen3.7 Max 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "qwen/qwen3.7-max",
        "Qwen3.7 Max",
        Some(1_000_000),
        Some(131_072),
        super::catalog::QWEN_PARAMETERS,
        ReasoningSupport::Supported,
        &[],
    )
}
