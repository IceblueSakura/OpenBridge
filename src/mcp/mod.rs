//! Model Context Protocol server facade and local tool extension boundary.
//!
//! The official `rmcp` SDK owns JSON-RPC encoding, protocol version negotiation
//! (stateless `2026-07-28` plus legacy `initialize` sessions), Streamable HTTP
//! transport, and tool dispatch. This module only wires the local `hello` tool
//! into a Tower service mounted by the ingress router behind the same static
//! downstream Bearer boundary.

mod tools;
mod transport;

pub(crate) use transport::{reject_origin, service};
