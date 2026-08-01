//! Tencent Hy3 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningLevel, ReasoningSupport};

/// 构造 Hy3 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "tencent/hy3",
        "Hy3",
        Some(262_144),
        Some(128_000),
        super::catalog::HY3_PARAMETERS,
        ReasoningSupport::Supported,
        &[
            ReasoningLevel::High,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ],
    )
}
