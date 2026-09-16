//! Aggregates canonical model facts for ChatGPT subscription profiles.
//!
//! These entries are explicit model profiles rather than runtime Provider discovery. They are
//! kept separate when ChatGPT exposes a model with context facts that differ from the general API.

use crate::registry::ModelConfig;

pub(crate) mod gpt_5_6_luna;
pub(crate) mod gpt_5_6_sol;
pub(crate) mod gpt_5_6_terra;
pub(crate) mod gpt_6_astra;

/// Returns all ChatGPT subscription model profiles compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        gpt_5_6_sol::config(),
        gpt_5_6_terra::config(),
        gpt_5_6_luna::config(),
        gpt_6_astra::config(),
    ]
}
