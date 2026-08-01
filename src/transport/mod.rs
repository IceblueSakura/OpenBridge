//! 上游网络传输与 SSE framing 边界。
//!
//! `sse` 只负责把字节流组装成事件，`upstream` 只负责将 adapter 生成的相对请求发送到
//! 已验证的 endpoint；协议语义、认证和重试决策由上层模块负责。

pub mod sse;
pub mod upstream;
