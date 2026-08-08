//! Canonical model facts for MiMo-V2.5-TTS.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
};

/// Stable OpenBridge catalog ID for MiMo-V2.5-TTS.
pub(crate) const ID: &str = "xiaomi/mimo-v2.5-tts";

/// Builds the provider-independent ordinary TTS model facts.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "MiMo-V2.5-TTS".to_owned(),
        description: Some("MiMo text-to-speech model exposed through Chat Completions.".to_owned()),
        context_length: ModelContextLength::new(Some(32_768), Some(32_768), Some(8_192)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Audio]),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: ["audio", "modalities", "temperature"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        reasoning: ReasoningSupport::Unsupported,
        reasoning_levels: Vec::new(),
    }
}
