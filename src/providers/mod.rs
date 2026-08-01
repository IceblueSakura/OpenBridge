//! OpenBridge 编译进二进制的 Provider 包入口。
//!
//! 具体 Provider adapter 各自拥有协议事实；编译目录装配集中在私有 catalog 模块。

mod catalog;

pub mod longcat;
pub mod openai;

pub use catalog::{REGISTRY_VERSION, build_compiled_registry, compiled_config};
