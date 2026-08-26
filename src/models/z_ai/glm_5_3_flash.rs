//! Complete canonical model facts for GLM-5.3-Flash (`z-ai/glm-5.3-flash`).
//!
//! Facts follow the exact OpenRouter endpoint record reverified on 2026-08-26:
//! <https://openrouter.ai/api/v1/models/z-ai/glm-5.3-flash/endpoints>.
//! OpenRouter's `stream_options` and `max_output_tokens` support are protocol-level controls
//! included in addition to the model-specific parameter record.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for GLM-5.3-Flash.
pub(crate) const ID: &str = "z-ai/glm-5.3-flash";

/// Builds the context, modality, parameter, and reasoning facts for GLM-5.3-Flash.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "GLM-5.3-Flash".to_owned(),
        description: Some(
            "Efficient native multimodal model for coding and long-horizon agent tasks.".to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_048_576),
                Some(1_048_576),
                Some(131_072),
            ),
            input_modalities: Some(vec![
                InputModality::Text,
                InputModality::Image,
                InputModality::Video,
            ]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "include_reasoning",
                "max_output_tokens",
                "max_tokens",
                "response_format",
                "stream_options",
                "temperature",
                "tool_choice",
                "tools",
                "top_k",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            reasoning: ReasoningProfile::supported([
                ReasoningLevel::Max,
                ReasoningLevel::High,
                ReasoningLevel::Low,
            ]),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glm_5_3_flash_accepts_chat_and_responses_output_limits() {
        let model = config();
        for parameter in ["max_tokens", "max_output_tokens", "stream_options"] {
            assert!(
                model
                    .supported_parameters()
                    .iter()
                    .any(|candidate| candidate == parameter),
                "missing {parameter}"
            );
        }
    }
}
