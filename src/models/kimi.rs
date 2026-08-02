//! Kimi 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod k3;

/// 返回 Kimi 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![k3::config()]
}
