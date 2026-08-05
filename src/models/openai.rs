//! Aggregates canonical model facts for OpenAI models.

use crate::registry::ModelConfig;

pub(crate) mod gpt_5_5;
pub(crate) mod gpt_5_6_luna;
pub(crate) mod gpt_5_6_sol;
pub(crate) mod gpt_5_6_terra;
pub(crate) mod text_embedding_3_small;

/// Returns the OpenAI generation model facts in their stable catalog order.
pub(crate) fn generation_configs() -> Vec<ModelConfig> {
    vec![
        gpt_5_6_sol::config(),
        gpt_5_6_terra::config(),
        gpt_5_6_luna::config(),
        gpt_5_5::config(),
    ]
}

/// Returns the OpenAI embedding model facts in their stable catalog order.
pub(crate) fn embedding_configs() -> Vec<ModelConfig> {
    vec![text_embedding_3_small::config()]
}
