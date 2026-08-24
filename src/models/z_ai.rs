//! Aggregates canonical model facts for the GLM family under `z-ai`.

use crate::registry::ModelConfig;

pub(crate) mod glm_5_2;
pub(crate) mod glm_5_3;

/// Returns all GLM model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![glm_5_2::config(), glm_5_3::config()]
}

#[cfg(test)]
mod tests {
    use crate::registry::{
        CanonicalModelTask, InputModality, ModelContextLength, OutputModality, ReasoningLevel,
    };

    #[test]
    fn configs_include_glm_5_3_openrouter_facts() {
        let model = super::configs()
            .into_iter()
            .find(|model| model.id == "z-ai/glm-5.3")
            .expect("GLM-5.3 must be present in the canonical catalog");

        assert_eq!(model.name, "GLM-5.3");
        assert_eq!(model.tokenizer.as_deref(), Some("Other"));
        assert_eq!(model.knowledge_cutoff, None);

        let CanonicalModelTask::Generation(profile) = model.task else {
            panic!("GLM-5.3 must be a generation model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(Some(1_048_576), Some(1_048_576), Some(131_072))
        );
        assert_eq!(profile.input_modalities, Some(vec![InputModality::Text]));
        assert_eq!(profile.output_modalities, Some(vec![OutputModality::Text]));
        assert_eq!(
            profile.supported_parameters,
            [
                "include_reasoning",
                "max_tokens",
                "response_format",
                "temperature",
                "tool_choice",
                "tools",
                "top_k",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        );
        assert_eq!(
            profile.reasoning.levels(),
            [
                ReasoningLevel::Max,
                ReasoningLevel::High,
                ReasoningLevel::Low
            ]
        );
    }
}
