//! Complete canonical model facts for Muse Spark 1.2 Contributor
//! (`meta/muse-spark-1.2-contributor`).
//!
//! Facts follow the OpenRouter model page and Models/Endpoints API records reverified on 2026-08-26:
//! <https://openrouter.ai/meta/muse-spark-1.2-contributor>. Recheck those dynamic records before
//! changing context, modality, parameter, or reasoning facts.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Meta Muse Spark 1.2 Contributor.
pub(crate) const ID: &str = "meta/muse-spark-1.2-contributor";

/// Builds the model facts published for Muse Spark 1.2 Contributor.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Meta Muse Spark 1.2 Contributor".to_owned(),
        description: Some(
            "Multimodal reasoning model for complex agentic, coding, and long-context workflows."
                .to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(
                Some(1_048_576),
                Some(1_048_576),
                Some(943_718),
            ),
            input_modalities: Some(vec![
                InputModality::Text,
                InputModality::Image,
                InputModality::Video,
                InputModality::File,
                InputModality::Audio,
            ]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "include_reasoning",
                "max_tokens",
                "repetition_penalty",
                "response_format",
                "structured_outputs",
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
                ReasoningLevel::XHigh,
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::Minimal,
            ]),
        }),
    }
}
