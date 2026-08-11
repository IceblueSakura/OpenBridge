//! Aggregates explicitly registered canonical model profiles.

use crate::registry::ModelConfig;

use super::{
    chatgpt, deepseek, google, meituan, minimax, moonshotai, nvidia, openai, qwen, xiaomi, z_ai,
};

/// Returns every explicitly registered canonical model profile compiled into the binary.
pub(crate) fn compiled_configs() -> Vec<ModelConfig> {
    [
        meituan::configs(),
        openai::generation_configs(),
        chatgpt::configs(),
        openai::embedding_configs(),
        deepseek::configs(),
        google::configs(),
        xiaomi::configs(),
        qwen::configs(),
        z_ai::configs(),
        moonshotai::configs(),
        minimax::configs(),
        nvidia::configs(),
    ]
    .concat()
}
