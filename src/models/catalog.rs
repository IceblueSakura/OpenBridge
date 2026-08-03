//! Aggregates provider-independent canonical model facts.

use crate::registry::ModelConfig;

use super::{deepseek, glm, gpt, hy, kimi, longcat, mimo, minimax, nemotron, qwen};

/// Returns every provider-independent model fact compiled into the binary.
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
