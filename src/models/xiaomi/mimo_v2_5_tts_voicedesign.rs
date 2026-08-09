//! Canonical model facts for MiMo-V2.5-TTS-VoiceDesign.

use crate::registry::{
    CanonicalModelTask, ModelConfig, ModelContextLength, VoiceDesignModelProfile,
};

/// Stable OpenBridge catalog ID for MiMo-V2.5-TTS-VoiceDesign.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5-tts-voicedesign";

/// Builds the provider-independent voice-design model facts.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5-TTS-VoiceDesign".to_owned(),
        description: Some(
            "MiMo voice-design synthesis model exposed through Chat Completions.".to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::VoiceDesign(VoiceDesignModelProfile {
            context_length: ModelContextLength::new(Some(32_768), Some(32_768), Some(8_192)),
            supported_parameters: [
                "audio",
                "modalities",
                "optimize_text_preview",
                "temperature",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }),
    }
}
