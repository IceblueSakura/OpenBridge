//! Compile-time entry point for the NVIDIA API Catalog Provider.
//!
//! The profile fixes the OpenAI-compatible endpoint, basic Chat wire contract, and explicitly
//! approved model Targets. Model registration cannot supply a request-selected endpoint or key.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
