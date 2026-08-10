//! Package entry point for downstream HTTP ingress.
//!
//! Submodules own Router/authentication, request lifecycle, OpenAI-compatible endpoints, the local
//! MCP test service, bounded attempts/fallback, Native/Bridged streaming, and response normalization.
//! This file only declares modules and exposes the service assembly entry point.

mod attempt;
mod auth;
mod credential_health;
mod forwarding;
mod handlers;
mod health;
mod lifecycle;
mod mcp;
mod openapi;
mod response;
mod router;
mod state;
mod streaming;

pub use router::build_router;
pub use state::GatewayState;
