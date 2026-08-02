//! OpenAI GPT 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod v5_3_codex_spark;
pub(crate) mod v5_5;
pub(crate) mod v5_6_luna;
pub(crate) mod v5_6_sol;
pub(crate) mod v5_6_terra;

/// 返回 GPT 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        v5_6_sol::config(),
        v5_6_terra::config(),
        v5_6_luna::config(),
        v5_5::config(),
        v5_3_codex_spark::config(),
    ]
}
