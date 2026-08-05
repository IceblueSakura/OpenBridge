//! Aggregates provider-independent canonical model facts.

use crate::registry::ModelConfig;

use super::{deepseek, meituan, minimax, moonshotai, openai, qwen, xiaomi, z_ai};

/// Returns every provider-independent model fact compiled into the binary.
pub(crate) fn compiled_configs() -> Vec<ModelConfig> {
    [
        meituan::configs(),
        openai::configs(),
        deepseek::configs(),
        xiaomi::configs(),
        qwen::configs(),
        z_ai::configs(),
        moonshotai::configs(),
        minimax::configs(),
    ]
        .concat()
}
