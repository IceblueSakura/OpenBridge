//! Aggregates canonical model facts for the Xiaomi MiMo family.

use crate::registry::ModelConfig;

pub(crate) mod v2_5;
pub(crate) mod v2_5_pro;

/// Returns all MiMo model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v2_5_pro::config(), v2_5::config()]
}
