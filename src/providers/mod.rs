//! Entry point for Providers compiled into the OpenBridge binary.
//!
//! Each Provider owns its static definitions; shared protocol mechanisms and catalog assembly remain private.

mod catalog;
pub(crate) mod openai_compatible;

pub mod deepseek;
pub mod longcat;
pub mod mimo;
pub mod openai;
pub mod openrouter;

pub use catalog::{REGISTRY_VERSION, build_compiled_registry, compiled_config};
