//! Conservative canonical model facts for Qwen Audio 3.0 ASR Flash.
//!
//! Speech-recognition capabilities follow the qwen-audio-3.0-asr-flash model-market snapshot
//! dated 2026-07-30. The profile does not infer native request parameters that are not published
//! in the shared model metadata shape.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
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
        context_length: ModelContextLength::new(None, None, None),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Audio]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: None,
        knowledge_cutoff: None,
        supported_parameters: Vec::new(),
        reasoning: ReasoningSupport::Unsupported,
        reasoning_levels: Vec::new(),
    }
}
