//! DeepSeek 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod v4;

/// 返回 DeepSeek 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    v4::configs()
}
