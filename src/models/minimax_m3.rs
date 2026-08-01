//! MiniMax M3 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningSupport};

/// 构造 MiniMax M3 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "minimax/minimax-m3",
        "MiniMax M3",
        Some(1_048_576),
        Some(512_000),
        super::catalog::MINIMAX_PARAMETERS,
        ReasoningSupport::Supported,
        &[],
    )
}
