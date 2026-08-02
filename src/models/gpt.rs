//! OpenAI GPT 家族的 canonical 模型聚合入口。

use crate::registry::ModelConfig;

pub(crate) mod v5_3;
pub(crate) mod v5_5;
pub(crate) mod v5_6;

/// 返回 GPT 家族所有编译进二进制的 canonical 模型事实。
pub(crate) fn configs() -> Vec<ModelConfig> {
    [v5_6::configs(), vec![v5_5::config(), v5_3::config()]].concat()
}
