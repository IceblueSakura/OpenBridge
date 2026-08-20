//! OpenBridge runtime library.
//!
//! This crate implements the native OpenAI-compatible forwarding baseline: bootstrap configuration
//! and the explicit code registry are compiled into an immutable registry at startup, while the
//! independent MCP module owns its local protocol and tool-extension boundary.

pub mod bridge;
pub mod config;
pub mod core;
pub mod credential;
pub(crate) mod execution;
pub mod identity;
pub mod ingress;
pub mod mcp;
pub mod models;
pub mod oauth2_credentials;
pub mod observability;
pub mod pipeline;
pub mod probe;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod transport;
pub mod upstream_credentials;
