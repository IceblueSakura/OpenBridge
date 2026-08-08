//! Canonical model catalog compiled into the OpenBridge binary.
//!
//! Model facts are explicit compile-time profiles rather than runtime Provider discovery; multiple
//! Upstream Targets can reference the same model ID, while each Upstream API supplies its upstream
//! model ID and more conservative protocol constraints. Most roots follow developer namespaces,
//! while `chatgpt` is reserved for subscription profiles whose verified limits differ from the
//! general API profile.

mod catalog;
pub(crate) mod chatgpt;
pub(crate) mod deepseek;
pub mod meituan;
pub(crate) mod minimax;
pub(crate) mod moonshotai;
pub(crate) mod openai;
pub(crate) mod qwen;
pub(crate) mod xiaomi;
pub(crate) mod z_ai;

pub(crate) use catalog::compiled_configs;
