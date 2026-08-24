//! Conservative canonical model facts for Qwen Audio 3.0 Realtime Plus.
//!
//! Qwen/DashScope documents text/audio input, text/audio output, Function Calling, and a native
//! realtime session limit of 50 audio turns or 300 seconds. The profile does not convert those
//! duration limits into token limits or invent Chat Completions parameter support.
//! Qwen-Audio does not support OpenAI-compatible Chat/Responses, so this model remains unrouted
//! until OpenBridge implements the corresponding DashScope Realtime WebSocket operation.

use crate::registry::{
    CanonicalModelTask, GenerationModelProfile, InputModality, ModelConfig, ModelContextLength,
    OutputModality, ReasoningProfile,
};

/// Stable OpenBridge catalog ID for Qwen Audio 3.0 Realtime Plus.
pub(crate) const ID: &str = "qwen/qwen-audio-3.0-realtime-plus";

/// Builds the provider-independent realtime speech-to-speech facts for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Audio 3.0 Realtime Plus".to_owned(),
        description: Some(
            "Full-duplex realtime voice model for low-latency audio conversations with text output and Function Calling."
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
