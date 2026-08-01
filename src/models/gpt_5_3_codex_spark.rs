//! GPT-5.3 Codex Spark 的 canonical 模型事实。

use crate::registry::{ModelConfig, ReasoningSupport};

/// 构造 LiteLLM 配置已确认的 GPT-5.3 Codex Spark 模型事实。
pub(crate) fn config() -> ModelConfig {
    super::catalog::model(
        "openai/gpt-5.3-codex-spark",
        "GPT-5.3 Codex Spark",
        None,
        None,
        &["reasoning"],
        ReasoningSupport::Supported,
        &[],
    )
}
