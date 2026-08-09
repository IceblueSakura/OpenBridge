//! Compile-time entry point for the DeepSeek Provider.
//!
//! Registers Chat Native surfaces for DeepSeek V4 Pro and Flash plus the confirmed Responses Native
//! surface for Flash.

mod definition;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
