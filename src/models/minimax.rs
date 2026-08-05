//! Aggregates canonical model facts for the MiniMax family.

use crate::registry::ModelConfig;

pub(crate) mod minimax_m3;

/// Returns all MiniMax model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![minimax_m3::config()]
}
