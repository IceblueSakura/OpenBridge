//! OpenBridge 编译进二进制的 canonical 模型目录。
//!
//! 模型事实与具体 Provider/endpoint 解耦；多个 Upstream Target 可以引用同一个模型 id，
//! 各 Upstream API 提供上游 model id 与更保守的协议级约束。

mod configured;
pub mod longcat;

use crate::registry::ModelConfig;

/// 返回所有编译进二进制、与 Provider 无关的模型事实。
pub(crate) fn compiled_configs() -> Vec<ModelConfig> {
    vec![configured::config(), longcat::config()]
}

pub(crate) use configured::MODEL_ID as CONFIGURED_MODEL_ID;
