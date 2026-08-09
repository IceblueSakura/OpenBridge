//! Compile-time entry point for ChatGPT subscription access through the Codex backend.
//!
//! The Provider remains independent from the OpenAI API-key Provider. Its OAuth registration,
//! Responses-only targets, and trusted client identity are fixed in code rather than selected by
//! configuration or requests.

mod definition;
pub(crate) mod oauth;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
