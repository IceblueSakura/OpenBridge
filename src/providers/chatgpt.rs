//! Compile-time entry point for ChatGPT subscription access through the Codex backend.
//!
//! The Provider remains independent from the OpenAI API-key Provider and registers only a disabled
//! target while OAuth login, refresh, and data-plane integration remain unimplemented.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
