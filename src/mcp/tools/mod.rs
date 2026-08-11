//! Static MCP tool catalog and dispatch boundary.
//!
//! Individual tool schemas, validation, and execution belong in leaf modules and must preserve the
//! no-egress boundary unless an explicitly approved tool contract states otherwise. The `rmcp`
//! `#[tool_router]` macro composes leaf tool handlers into the `HelloServer` handler used by the
//! transport service.

mod hello;

pub(crate) use hello::HelloServer;
