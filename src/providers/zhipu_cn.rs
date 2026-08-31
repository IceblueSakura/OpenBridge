//! Compile-time entry point for the Zhipu AI China Provider.
//!
//! The profile fixes the Chinese OpenAI-compatible Chat and GLM-5.3 Responses paths plus approved
//! GLM Targets. Model registration cannot supply a request-selected endpoint or key.

mod definition;
mod media;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
