//! Aggregates canonical model facts for the GLM family.

use crate::registry::ModelConfig;

pub(crate) mod v5_2;

/// Returns all GLM model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v5_2::config()]
}
