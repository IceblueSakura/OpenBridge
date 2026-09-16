//! Aggregates canonical model facts for the Google family.

use crate::registry::ModelConfig;

pub(crate) mod gemini_3_8_flash;

/// Returns all Google model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![gemini_3_8_flash::config()]
}
