//! OpenBridge 编译进二进制的 Provider 包入口。
//!
//! 具体 Provider 各自拥有静态定义；共享协议机制与编译目录装配保持私有。

mod catalog;
pub(crate) mod openai_compatible;

pub mod deepseek;
pub mod longcat;
pub mod mimo;
pub mod openai;

pub use catalog::{REGISTRY_VERSION, build_compiled_registry, compiled_config};
