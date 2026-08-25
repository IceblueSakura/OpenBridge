//! Aggregates canonical model facts for the DeepSeek family.

use crate::registry::ModelConfig;

pub(crate) mod deepseek_v4_flash;
pub(crate) mod deepseek_v4_flash_vision_exp;
pub(crate) mod deepseek_v4_pro;

/// Returns all DeepSeek model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        deepseek_v4_pro::config(),
        deepseek_v4_flash::config(),
        deepseek_v4_flash_vision_exp::config(),
    ]
}
