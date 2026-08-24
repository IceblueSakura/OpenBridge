//! Aggregates canonical model facts for the Google family.

use crate::registry::ModelConfig;

pub(crate) mod gemini_3_7_flash;
pub(crate) mod gemma_4_31b_it;

/// Returns all Google model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![gemma_4_31b_it::config(), gemini_3_7_flash::config()]
}
