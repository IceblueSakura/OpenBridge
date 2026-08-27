//! Entry point for Providers compiled into the OpenBridge binary.
//!
//! Each Provider owns its static definitions; shared protocol mechanisms and catalog assembly remain private.

mod catalog;
pub(crate) mod openai_compatible;

pub mod bailian;
pub mod chatgpt;
pub mod deepseek;
pub mod kimi_cn;
pub mod longcat;
pub mod mimo;
pub mod nvidia;
pub mod openai;
pub mod openrouter;
pub mod zhipu_cn;

pub use catalog::{
    REGISTRY_VERSION, build_compiled_registry, build_compiled_registry_with_active_pools,
    compiled_config,
};
