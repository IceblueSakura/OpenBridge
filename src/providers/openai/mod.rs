//! OpenAI Provider 的编译期实现入口。
//!
//! 静态能力契约、协议 adapter 与 target/upstream API 注册分别由独立子模块拥有。

mod adapter;
mod contract;
mod registration;

pub use adapter::OpenAiAdapter;
pub use contract::CONTRACT;
pub use registration::{conservative_openai_capabilities, upstream_targets};
