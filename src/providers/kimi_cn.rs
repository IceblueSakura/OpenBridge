//! Compile-time entry point for the Moonshot Kimi China Provider.
//!
//! The profile fixes the Chinese OpenAI-compatible endpoint, a conservative Chat Native wire
//! contract, and the explicitly approved Kimi K3 Target. Model registration cannot supply a
//! request-selected endpoint or key.

mod definition;
mod media;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
