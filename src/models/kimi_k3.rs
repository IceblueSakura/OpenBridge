//! Kimi K3 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningLevel, ReasoningSupport};

/// 构造 Kimi K3 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "moonshotai/kimi-k3",
        "Kimi K3",
        Some(1_048_576),
        None,
        super::catalog::KIMI_PARAMETERS,
        ReasoningSupport::Supported,
        &[
            ReasoningLevel::Max,
            ReasoningLevel::High,
            ReasoningLevel::Low,
        ],
    )
}
