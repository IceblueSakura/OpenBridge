//! Conservative canonical model facts for Qwen Audio 3.0 ASR Flash.
//!
//! Speech-recognition capabilities follow the qwen-audio-3.0-asr-flash model-market snapshot
//! dated 2026-07-30. The profile does not infer native request parameters that are not published
//! in the shared model metadata shape.
//! Qwen-Audio does not support OpenAI-compatible Chat/Responses, so this model remains unrouted
//! until OpenBridge implements the corresponding DashScope-native ASR operation and transport.

use crate::registry::{
    CanonicalModelTask, ModelConfig, ModelContextLength, SpeechRecognitionModelProfile,
};

/// Stable OpenBridge catalog ID for Qwen Audio 3.0 ASR Flash.
pub(crate) const ID: &str = "qwen/qwen-audio-3.0-asr-flash";

/// Builds the provider-independent speech-recognition facts confirmed for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Audio 3.0 ASR Flash".to_owned(),
        description: Some(
            "Qwen Audio 3.0 Flash speech-recognition model for multilingual audio transcription."
                .to_owned(),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        task: CanonicalModelTask::SpeechRecognition(SpeechRecognitionModelProfile {
            context_length: ModelContextLength::new(None, None, None),
            supported_parameters: Vec::new(),
        }),
    }
}
