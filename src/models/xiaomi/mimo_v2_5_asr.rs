//! Canonical model facts for MiMo-V2.5-ASR.

use crate::registry::{
    CanonicalModelTask, ModelConfig, ModelContextLength, SpeechRecognitionModelProfile,
};

/// Stable OpenBridge catalog ID for MiMo-V2.5-ASR.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5-asr";

/// Builds the provider-independent speech-recognition model facts.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5-ASR".to_owned(),
        description: Some(
            "MiMo speech-recognition model exposed through Chat Completions.".to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::SpeechRecognition(SpeechRecognitionModelProfile {
            context_length: ModelContextLength::new(Some(32_768), Some(32_768), Some(8_192)),
            supported_parameters: ["asr_options", "max_tokens", "temperature"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}
