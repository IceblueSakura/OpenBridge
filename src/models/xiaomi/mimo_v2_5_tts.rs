//! Canonical model facts for MiMo-V2.5-TTS.

use crate::registry::{
    CanonicalModelTask, ModelConfig, ModelContextLength, SpeechSynthesisModelProfile,
};

/// Stable OpenBridge catalog ID for MiMo-V2.5-TTS.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5-tts";

/// Builds the provider-independent ordinary TTS model facts.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5-TTS".to_owned(),
        description: Some("MiMo text-to-speech model exposed through Chat Completions.".to_owned()),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::SpeechSynthesis(SpeechSynthesisModelProfile {
            context_length: ModelContextLength::new(Some(32_768), Some(32_768), Some(8_192)),
            supported_parameters: ["audio", "modalities", "temperature"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}
