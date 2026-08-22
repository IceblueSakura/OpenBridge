//! Compile-time entry point for the Alibaba Cloud Model Studio Provider.
//!
//! The profile fixes the Beijing OpenAI-compatible endpoint, bounded Chat and Embeddings wire
//! contracts, and explicitly approved model Targets. Model registration cannot supply a
//! request-selected endpoint or key.

mod definition;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{native_provider_instance, provider_instance, upstream_targets};
