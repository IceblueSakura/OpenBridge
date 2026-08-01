//! NVIDIA Nemotron 3 Ultra 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningLevel, ReasoningSupport};

/// 构造 Nemotron 3 Ultra 550B A55B 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "nvidia/nemotron-3-ultra-550b-a55b",
        "Nemotron 3 Ultra 550B A55B",
        Some(1_000_000),
        Some(65_536),
        super::catalog::NEMOTRON_PARAMETERS,
        ReasoningSupport::Supported,
        &[ReasoningLevel::High, ReasoningLevel::Medium],
    )
}
