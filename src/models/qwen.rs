//! Aggregates canonical model facts for the Qwen family.

use crate::registry::ModelConfig;

pub(crate) mod qwen3_5_livetranslate_flash_realtime;
pub(crate) mod qwen3_6_27b;
pub(crate) mod qwen3_7_max;
pub(crate) mod qwen3_7_plus;
pub(crate) mod qwen3_8_max;
pub(crate) mod qwen_image_2_0_pro;

/// Returns all Qwen model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        qwen3_7_max::config(),
        qwen3_7_plus::config(),
        qwen3_8_max::config(),
        qwen_image_2_0_pro::config(),
        qwen3_5_livetranslate_flash_realtime::config(),
        qwen3_6_27b::config(),
    ]
}
