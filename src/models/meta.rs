//! Aggregates canonical model facts for the Meta family.

use crate::registry::ModelConfig;

pub(crate) mod muse_spark_1_2_contributor;

/// Returns all Meta model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![muse_spark_1_2_contributor::config()]
}
