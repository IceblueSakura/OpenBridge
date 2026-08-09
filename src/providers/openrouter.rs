//! Compile-time entry point for the OpenRouter Provider.
//!
//! Registers Chat and stateless Responses Native surfaces for approved DeepSeek and MiniMax models;
//! server-side state, dynamic routing fields, and optional OpenRouter attribution headers are outside
//! the committed boundary.

mod definition;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
