//! Package entry point for downstream HTTP ingress.
//!
//! Submodules own Router/authentication, request lifecycle, OpenAI-compatible endpoints, MCP route
//! integration, bounded attempts/fallback, Native/Bridged streaming, and response normalization.
//! The independent crate-level `mcp` module owns MCP protocol and tool behavior.

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
