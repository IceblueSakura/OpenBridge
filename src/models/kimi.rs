//! Aggregates canonical model facts for the Kimi family.

use crate::registry::ModelConfig;

pub(crate) mod k3;

/// Returns all Kimi model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![k3::config()]
}
