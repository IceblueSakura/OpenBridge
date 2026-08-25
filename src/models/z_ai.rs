//! Aggregates canonical model facts for the GLM family under `z-ai`.

use crate::registry::ModelConfig;

pub(crate) mod glm_5_2;
pub(crate) mod glm_5_3;

/// Returns all GLM model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![glm_5_2::config(), glm_5_3::config()]
}
