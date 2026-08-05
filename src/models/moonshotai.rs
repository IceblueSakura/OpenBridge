//! Aggregates canonical model facts for the Kimi family under `moonshotai`.

use crate::registry::ModelConfig;

pub(crate) mod kimi_k3;

/// Returns all Kimi model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![kimi_k3::config()]
}
