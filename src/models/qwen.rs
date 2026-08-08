//! Aggregates canonical model facts for the Qwen family.

use crate::registry::ModelConfig;

pub(crate) mod qwen3_5_livetranslate_flash_realtime;
pub(crate) mod qwen3_6_27b;
pub(crate) mod qwen3_7_max;
pub(crate) mod qwen3_7_plus;
pub(crate) mod qwen3_7_text_embedding;
pub(crate) mod qwen3_8_max;
pub(crate) mod qwen_audio_3_0_asr_flash;
pub(crate) mod qwen_image_2_0_pro;
pub(crate) mod qwen_image_3_0;
pub(crate) mod qwen_image_3_0_pro;

/// Returns all Qwen model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        qwen3_7_max::config(),
        qwen3_7_plus::config(),
        qwen3_7_text_embedding::config(),
        qwen3_8_max::config(),
        qwen_image_2_0_pro::config(),
        qwen_image_3_0::config(),
        qwen_image_3_0_pro::config(),
        qwen_audio_3_0_asr_flash::config(),
        qwen3_5_livetranslate_flash_realtime::config(),
        qwen3_6_27b::config(),
    ]
}
