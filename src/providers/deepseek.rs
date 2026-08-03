//! Compile-time entry point for the DeepSeek Provider.
//!
//! Registers Chat Native surfaces for DeepSeek V4 Pro and Flash; compiled Routes explicitly bridge
//! Responses requests to Chat.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::upstream_targets;
