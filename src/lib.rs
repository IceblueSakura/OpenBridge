//! OpenBridge 的运行时库。
//!
//! 当前 crate 实现 OpenAI-compatible 的原生转发基线：配置在启动或显式 reload 时
//! 解析为不可变 snapshot，HTTP 热路径只读取该 snapshot；具体 provider 行为留在
//! 编译期 adapter 中，避免把认证、路由或协议规则变成客户端可控配置。

pub mod config;
pub mod core;
pub mod ingress;
pub mod pipeline;
pub mod provider;
pub mod transport;
