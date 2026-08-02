//! LongCat Provider 的编译期实现入口。
//!
//! 静态能力契约、协议 adapter 与 target/upstream API 注册分别由独立子模块拥有。

mod adapter;
mod contract;
mod registration;

pub use adapter::LongCatAdapter;
pub(crate) use contract::CONTRACT;
pub(crate) use registration::upstream_targets;
