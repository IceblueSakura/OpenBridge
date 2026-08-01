//! MiMo-V2.5-Pro 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningSupport};

/// 构造 MiMo-V2.5-Pro 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "xiaomi/mimo-v2.5-pro",
        "MiMo-V2.5-Pro",
        Some(1_050_000),
        Some(131_072),
        super::catalog::MIMO_PARAMETERS,
        ReasoningSupport::Supported,
        &[],
    )
}
