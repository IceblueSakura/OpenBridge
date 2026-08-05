//! Compile-time entry point for the DeepSeek Provider.
//!
//! Registers Chat Native surfaces for DeepSeek V4 Pro and Flash; Responses for Flash is supplied by
//! the explicit OpenRouter route source rather than by a DeepSeek Protocol Bridge.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::upstream_targets;
