//! Aggregates canonical model facts for the Tencent HY family.

use crate::registry::ModelConfig;

pub(crate) mod v3;

/// Returns all HY model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v3::config()]
}
