//! OpenRouter Provider 的编译期实现入口。
//!
//! 当前注册 Nemotron 3 Ultra 的 Chat 与无状态 Responses Native surface；服务端状态、
//! 动态路由字段和 OpenRouter 可选归因 header 不在已承诺边界内。

mod definition;
mod registration;

pub(crate) use definition::ADAPTER;
pub use definition::CONTRACT;
pub(crate) use registration::upstream_targets;
