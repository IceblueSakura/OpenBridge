//! Qwen 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod v3_7_max;
pub(crate) mod v3_7_plus;

/// 返回 Qwen 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v3_7_max::config(), v3_7_plus::config()]
}
