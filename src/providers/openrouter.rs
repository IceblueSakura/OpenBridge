//! Compile-time entry point for the OpenRouter Provider.
//!
//! Registers Chat and stateless Responses Native surfaces for Nemotron 3 Ultra; server-side state,
//! dynamic routing fields, and optional OpenRouter attribution headers are outside the committed boundary.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::upstream_targets;
