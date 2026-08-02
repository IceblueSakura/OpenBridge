//! Xiaomi MiMo Provider 的编译期实现入口。
//!
//! 当前注册 MiMo V2.5 Pro/V2.5 的 Chat 与无状态 Responses Native surface。

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::upstream_targets;
