//! Aggregates canonical model facts for the NVIDIA Nemotron family.

use crate::registry::ModelConfig;

pub(crate) mod v3;

/// Returns all Nemotron model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v3::ultra()]
}
