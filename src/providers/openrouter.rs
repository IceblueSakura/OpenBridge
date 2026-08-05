//! Compile-time entry point for the OpenRouter Provider.
//!
//! Registers Chat and stateless Responses Native surfaces for DeepSeek V4 Flash; server-side state,
//! dynamic routing fields, and optional OpenRouter attribution headers are outside the committed boundary.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
