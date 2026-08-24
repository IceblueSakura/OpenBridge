//! Compile-time entry point for the DeepSeek Provider.
//!
//! Registers confirmed Chat and Responses Native surfaces for DeepSeek V4 Pro, Flash, and Vision.

mod definition;
mod media;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
