//! Compile-time entry point for the OpenRouter Provider.
//!
//! Registers model-specific Chat and stateless Responses Native surfaces for the approved OpenRouter
//! catalog; server-side state, dynamic routing fields, and optional attribution headers remain
//! outside the committed boundary.

mod definition;
mod media;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
