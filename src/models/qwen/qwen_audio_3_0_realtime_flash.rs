//! Conservative canonical model facts for Qwen Audio 3.0 Realtime Flash.
//!
//! Qwen/DashScope documents the same text/audio modalities and native realtime session limits as
//! Realtime Plus: up to 50 retained audio turns or 300 seconds of audio. Those limits are not token
//! counts, so this profile keeps token limits unknown. The native Realtime API parameters are not
//! represented as Chat Completions parameters.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Qwen Audio 3.0 Realtime Flash.
pub(crate) const ID: &str = "qwen/qwen-audio-3.0-realtime-flash";

/// Builds the provider-independent realtime audio facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Audio 3.0 Realtime Flash".to_owned(),
        description: Some(
            "Low-latency Qwen full-duplex voice model with streaming text and audio output."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::Generation(GenerationModelProfile {
            context_length: ModelContextLength::new(None, None, None),
            input_modalities: Some(vec![InputModality::Text, InputModality::Audio]),
            output_modalities: Some(vec![OutputModality::Text, OutputModality::Audio]),
            supported_parameters: Vec::new(),
            reasoning: ReasoningProfile::Unsupported,
        }),
    }
}
