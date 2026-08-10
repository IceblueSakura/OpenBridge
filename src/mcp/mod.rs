//! Model Context Protocol server facade and local tool extension boundary.
//!
//! Streamable HTTP protocol handling is isolated from the static tool registry. Each tool owns a
//! leaf module so new tools can be added without coupling their schemas or execution logic to the
//! transport, OpenAI-compatible ingress, registry, pipeline, or Provider adapters.

mod tools;
mod transport;

pub(crate) use transport::{endpoint, reject_origin};
