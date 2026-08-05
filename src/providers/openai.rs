//! Compile-time entry point for the OpenAI Provider.
//!
//! Static Provider definitions and target/Upstream API registrations are owned by separate submodules.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub use registration::{conservative_openai_capabilities, provider_instance, upstream_targets};
