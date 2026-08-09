//! Compile-time entry point for the LongCat Provider.
//!
//! Static Provider definitions and target/Upstream API registrations are owned by separate submodules.

mod definition;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
