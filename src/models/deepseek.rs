//! Aggregates canonical model facts for the DeepSeek family.

use crate::registry::ModelConfig;

pub(crate) mod v4_flash;
pub(crate) mod v4_pro;

/// Returns all DeepSeek model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v4_pro::config(), v4_flash::config()]
}
