//! 与 Provider 无关的 canonical 模型目录聚合。

use crate::registry::ModelConfig;

use super::{deepseek, glm, gpt, hy, kimi, longcat, mimo, minimax, nemotron, qwen};

/// 返回所有编译进二进制、与 Provider 无关的模型事实。
pub(crate) fn compiled_configs() -> Vec<ModelConfig> {
    [
        longcat::configs(),
        gpt::configs(),
        deepseek::configs(),
        mimo::configs(),
        qwen::configs(),
        glm::configs(),
        kimi::configs(),
        minimax::configs(),
        hy::configs(),
        nemotron::configs(),
    ]
    .concat()
}
