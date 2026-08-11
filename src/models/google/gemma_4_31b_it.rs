//! Conservative canonical model facts for Google Gemma 4 31B Instruct.
//!
//! Facts follow the 2026-08-10 direct probe of the OpenRouter free endpoint
//! (`google/gemma-4-31b-it:free`): text chat, stream+usage, parallel tool calls,
//! PNG data-URL image input and the Responses endpoint all work; JSON-schema
//! strict output is unreliable (markdown-wrapped JSON) and reasoning is off by
//! default, so only those confirmed facts are recorded.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningLevel, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Gemma 4 31B Instruct.
pub(crate) const ID: &str = "google/gemma-4-31b-it";

/// Builds the provider-independent model facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Google Gemma 4 31B Instruct".to_owned(),
        description: Some(
            "Google DeepMind open multimodal instruction model with text and image input."
                .to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(Some(262_144), Some(262_144), None),
            input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
            output_modalities: Some(vec![OutputModality::Text]),
            supported_parameters: [
                "frequency_penalty",
                "max_tokens",
                "presence_penalty",
                "response_format",
                "seed",
                "stop",
                "stream_options",
                "temperature",
                "tool_choice",
                "tools",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            reasoning: ReasoningProfile::supported([ReasoningLevel::None]),
        }),
    }
}
