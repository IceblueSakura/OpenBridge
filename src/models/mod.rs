//! OpenBridge 编译进二进制的 canonical 模型目录。
//!
//! 模型事实与具体 Provider/endpoint 解耦；多个 Upstream Target 可以引用同一个模型 id，
//! 各 Upstream API 提供上游 model id 与更保守的协议级约束。

mod catalog;
mod configured;
mod deepseek_v4_flash;
mod deepseek_v4_pro;
mod glm_5_2;
mod gpt_5_3_codex_spark;
mod gpt_5_5;
mod gpt_5_6_luna;
mod gpt_5_6_sol;
mod gpt_5_6_terra;
mod hy3;
mod kimi_k3;
pub mod longcat_2_0;
mod mimo_v2_5;
mod mimo_v2_5_pro;
mod minimax_m3;
mod nemotron_3_ultra;
mod qwen3_7_max;
mod qwen3_7_plus;

use crate::registry::ModelConfig;

/// 返回所有编译进二进制、与 Provider 无关的模型事实。
pub(crate) fn compiled_configs() -> Vec<ModelConfig> {
    vec![
        configured::config(),
        longcat_2_0::config(),
        gpt_5_6_sol::config(),
        gpt_5_6_terra::config(),
        gpt_5_6_luna::config(),
        gpt_5_5::config(),
        gpt_5_3_codex_spark::config(),
        deepseek_v4_pro::config(),
        deepseek_v4_flash::config(),
        mimo_v2_5_pro::config(),
        mimo_v2_5::config(),
        qwen3_7_max::config(),
        qwen3_7_plus::config(),
        glm_5_2::config(),
        kimi_k3::config(),
        minimax_m3::config(),
        hy3::config(),
        nemotron_3_ultra::config(),
    ]
}

pub(crate) use configured::MODEL_ID as CONFIGURED_MODEL_ID;
