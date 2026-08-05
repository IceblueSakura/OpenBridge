//! Compile-time entry point for the Xiaomi MiMo Provider.
//!
//! Registers Chat and stateless Responses Native surfaces for MiMo V2.5 Pro and V2.5.

mod definition;
mod registration;

pub use definition::CONTRACT;
pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
