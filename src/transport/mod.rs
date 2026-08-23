//! Upstream network transport and SSE framing boundaries.
//!
//! `sse` assembles byte streams into events, while `upstream` sends adapter-generated relative
//! requests to validated endpoints. Higher layers own protocol semantics, authentication, and retry decisions.

mod error;
pub mod sse;
pub mod upstream;

pub(crate) use error::is_timeout_error;
