//! OpenBridge 的运行时库。
//!
//! 当前 crate 实现 OpenAI-compatible 的原生转发基线：bootstrap 配置与显式代码注册表
//! 在启动时编译为不可变 snapshot，HTTP 热路径只读取该 snapshot。

pub mod config;
pub mod core;
pub mod ingress;
pub mod pipeline;
pub mod probe;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod transport;
