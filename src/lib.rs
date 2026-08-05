//! OpenBridge runtime library.
//!
//! This crate implements the native OpenAI-compatible forwarding baseline: bootstrap configuration
//! and the explicit code registry are compiled into an immutable registry at startup, and the
//! HTTP hot path reads only that registry.

pub mod bridge;
pub mod codex_auth;
pub mod codex_identity;
pub mod config;
pub mod core;
pub mod credential;
pub mod identity;
pub mod ingress;
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
