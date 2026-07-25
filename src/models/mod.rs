//! OpenBridge 编译进二进制的 canonical 模型目录。
//!
//! 模型事实与具体 Provider/endpoint 解耦；多个 deployment 可以引用同一个模型 id，并
//! 各自提供上游 model id、endpoint、credential 与更保守的 deployment constraint。

mod configured;
pub mod longcat;

use crate::registry::ModelDefinition;

/// 返回所有编译进二进制、与 Provider 无关的模型事实。
pub(crate) fn compiled_definitions() -> Vec<ModelDefinition> {
    vec![configured::definition(), longcat::definition()]
}

pub(crate) use configured::MODEL_ID as CONFIGURED_MODEL_ID;
