//! MiniMax 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod m3;

/// 返回 MiniMax 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![m3::config()]
}
