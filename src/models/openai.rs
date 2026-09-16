//! Aggregates canonical model facts for OpenAI models.

use crate::registry::ModelConfig;

pub(crate) mod gpt_5_6_luna;
pub(crate) mod gpt_5_6_sol;
pub(crate) mod gpt_5_6_terra;

/// Returns the OpenAI generation model facts in their stable catalog order.
pub(crate) fn generation_configs() -> Vec<ModelConfig> {
    vec![
        gpt_5_6_sol::config(),
        gpt_5_6_terra::config(),
        gpt_5_6_luna::config(),
    ]
}
