//! LongCat Provider 的编译期实现入口。
//!
//! 静态 Provider 定义与 target/upstream API 注册分别由独立子模块拥有。

mod definition;
mod registration;

pub(crate) use definition::{CONTRACT, DEFINITION};
pub(crate) use registration::upstream_targets;
