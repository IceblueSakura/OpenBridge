//! Compile-time entry point for the Xiaomi MiMo Provider.
//!
//! Registers dual-protocol text/image surfaces and Chat-only audio task surfaces for MiMo V2.5.

mod definition;
mod media;
mod registration;

pub(crate) use definition::DEFINITION;
pub(crate) use registration::{provider_instance, upstream_targets};
