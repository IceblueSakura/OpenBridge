//! Xiaomi MiMo 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod v2_5;

/// 返回 MiMo 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    v2_5::configs()
}
