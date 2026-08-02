//! DeepSeek Provider 的编译期实现入口。
//!
//! 当前注册 DeepSeek V4 Pro/Flash 的 Chat Native surface；Responses 由编译 Route 显式 bridge 到 Chat。

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::upstream_targets;
