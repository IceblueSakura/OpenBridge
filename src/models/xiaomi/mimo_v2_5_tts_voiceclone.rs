//! Canonical model facts for MiMo-V2.5-TTS-VoiceClone.

use crate::registry::{
    CanonicalModelTask, ModelConfig, ModelContextLength, VoiceCloneModelProfile,
};

/// Stable OpenBridge catalog ID for MiMo-V2.5-TTS-VoiceClone.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5-tts-voiceclone";

/// Builds the provider-independent voice-clone model facts.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5-TTS-VoiceClone".to_owned(),
        description: Some(
            "MiMo reference-voice cloning model exposed through Chat Completions.".to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::VoiceClone(VoiceCloneModelProfile {
            context_length: ModelContextLength::new(Some(32_768), Some(32_768), Some(8_192)),
            supported_parameters: ["audio", "modalities", "temperature"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }),
    }
}
