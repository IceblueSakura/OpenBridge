//! Aggregates canonical model facts for the DeepSeek family.

use crate::registry::ModelConfig;

pub(crate) mod deepseek_v4_flash;
pub(crate) mod deepseek_v4_flash_vision_exp;
pub(crate) mod deepseek_v4_pro;

/// Returns all DeepSeek model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        deepseek_v4_pro::config(),
        deepseek_v4_flash::config(),
        deepseek_v4_flash_vision_exp::config(),
    ]
}

#[cfg(test)]
mod tests {
    use crate::registry::{
        CanonicalModelTask, InputModality, ModelContextLength, OutputModality, ReasoningLevel,
    };

    #[test]
    fn configs_include_deepseek_v4_flash_vision_exp_openrouter_facts() {
        let model = super::configs()
            .into_iter()
            .find(|model| model.id == "deepseek/deepseek-v4-flash-vision-exp")
            .expect("DeepSeek V4 Flash Vision Exp must be present in the canonical catalog");

        assert_eq!(model.name, "DeepSeek V4 Flash Vision Exp");
        assert_eq!(model.tokenizer.as_deref(), Some("DeepSeek"));
        assert_eq!(model.knowledge_cutoff, None);

        let CanonicalModelTask::Generation(profile) = model.task else {
            panic!("DeepSeek V4 Flash Vision Exp must be a generation model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(Some(1_048_576), Some(1_048_576), Some(384_000))
        );
        assert_eq!(
            profile.input_modalities,
            Some(vec![InputModality::Text, InputModality::Image])
        );
        assert_eq!(profile.output_modalities, Some(vec![OutputModality::Text]));
        assert_eq!(
            profile.supported_parameters,
            [
                "frequency_penalty",
                "include_reasoning",
                "logprobs",
                "max_tokens",
                "presence_penalty",
                "response_format",
                "stop",
                "temperature",
                "tool_choice",
                "tools",
                "top_logprobs",
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
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]
        );
    }
}
