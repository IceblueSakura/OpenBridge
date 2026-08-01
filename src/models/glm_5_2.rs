//! GLM-5.2 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningLevel, ReasoningSupport};

/// 构造 GLM-5.2 的 context、参数和 reasoning 事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "z-ai/glm-5.2",
        "GLM-5.2",
        Some(1_048_576),
        Some(131_072),
        super::catalog::GLM_PARAMETERS,
        ReasoningSupport::Supported,
        &[ReasoningLevel::XHigh, ReasoningLevel::High],
    )
}
