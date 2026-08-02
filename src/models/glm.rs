//! GLM 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod v5_2;

/// 返回 GLM 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v5_2::config()]
}
