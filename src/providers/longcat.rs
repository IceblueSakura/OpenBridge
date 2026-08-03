//! Compile-time entry point for the LongCat Provider.
//!
//! Static Provider definitions and target/Upstream API registrations are owned by separate submodules.

mod definition;
mod registration;

pub(crate) use definition::{CONTRACT, DEFINITION};
pub(crate) use registration::upstream_targets;
