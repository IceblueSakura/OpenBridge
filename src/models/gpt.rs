//! Aggregates canonical model facts for the OpenAI GPT family.

use crate::registry::ModelConfig;

pub(crate) mod v5_3_codex_spark;
pub(crate) mod v5_5;
pub(crate) mod v5_6_luna;
pub(crate) mod v5_6_sol;
pub(crate) mod v5_6_terra;

/// Returns all GPT model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        v5_6_sol::config(),
        v5_6_terra::config(),
        v5_6_luna::config(),
        v5_5::config(),
        v5_3_codex_spark::config(),
    ]
}
