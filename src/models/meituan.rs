//! Aggregates canonical model facts for the LongCat family under `meituan`.

use crate::registry::ModelConfig;

pub(crate) mod longcat_2_0;

/// Returns all LongCat model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![longcat_2_0::config()]
}
