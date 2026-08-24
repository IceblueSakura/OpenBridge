//! Aggregates canonical model facts for the xAI family.

use crate::registry::ModelConfig;

pub(crate) mod grok_4_6;

/// Returns all xAI model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![grok_4_6::config()]
}
