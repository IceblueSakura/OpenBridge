//! Aggregates canonical model facts for OpenAI models.

use crate::registry::ModelConfig;

pub(crate) mod gpt_5_3_codex_spark;
pub(crate) mod gpt_5_5;
pub(crate) mod gpt_5_6_luna;
pub(crate) mod gpt_5_6_sol;
pub(crate) mod gpt_5_6_terra;
pub(crate) mod text_embedding_3_small;

/// Returns all OpenAI model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        gpt_5_6_sol::config(),
        gpt_5_6_terra::config(),
        gpt_5_6_luna::config(),
        gpt_5_5::config(),
        gpt_5_3_codex_spark::config(),
        text_embedding_3_small::config(),
    ]
}
