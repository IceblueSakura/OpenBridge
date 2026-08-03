//! OpenAI-compatible HTTP ingress 的包入口。
//!
//! 子模块分别拥有 Router/认证、请求生命周期、endpoint、有限 attempt/fallback、
//! Native/Bridged streaming 和响应归一化。本文件仅声明模块并暴露服务装配入口。

mod attempt;
mod auth;
mod credential_health;
mod forwarding;
mod handlers;
mod health;
mod lifecycle;
mod openapi;
mod response;
mod router;
mod state;
mod streaming;

pub use router::build_router;
pub use state::GatewayState;
