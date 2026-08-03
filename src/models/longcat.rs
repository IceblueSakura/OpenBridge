//! Aggregates canonical model facts for the LongCat family.

use crate::registry::ModelConfig;

pub(crate) mod v2;

/// Returns all LongCat model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![v2::config()]
}
