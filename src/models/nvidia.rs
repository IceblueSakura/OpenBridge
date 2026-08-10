//! Aggregates canonical model facts for the NVIDIA family.

use crate::registry::ModelConfig;

pub(crate) mod nemotron_3_embed_1b;

/// Returns all NVIDIA model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![nemotron_3_embed_1b::config()]
}
