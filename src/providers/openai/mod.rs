//! OpenAI Provider 的编译期实现入口。
//!
//! 静态 Provider 定义与 target/upstream API 注册分别由独立子模块拥有。

mod definition;
mod registration;

pub(crate) use definition::ADAPTER;
pub use definition::CONTRACT;
pub use registration::{conservative_openai_capabilities, upstream_targets};
