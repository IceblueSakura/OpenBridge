//! Aggregates canonical model facts for the Qwen family.

use crate::registry::ModelConfig;

pub(crate) mod v3_7_max;
pub(crate) mod v3_7_plus;

/// Returns all Qwen model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v3_7_max::config(), v3_7_plus::config()]
}
