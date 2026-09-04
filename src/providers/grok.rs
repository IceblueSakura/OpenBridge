//! Compile-time entry point for Grok subscription access through the fixed CLI proxy backend.
//!
//! The Provider remains independent from the xAI API-key surface. Its OAuth registration,
//! Responses-only target, and trusted client identity are fixed in code rather than selected by
//! configuration or requests.

mod definition;
pub(crate) mod oauth;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
