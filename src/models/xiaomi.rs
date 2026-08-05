//! Aggregates canonical model facts for the MiMo family under `xiaomi`.

use crate::registry::ModelConfig;

pub(crate) mod mimo_v2_5;
pub(crate) mod mimo_v2_5_pro;

/// Returns all MiMo model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![mimo_v2_5_pro::config(), mimo_v2_5::config()]
}
