//! Canonical model facts for MiMo-V2.5-ASR.

use crate::registry::{
    InputModality, ModelConfig, ModelContextLength, ModelMode, OutputModality, ReasoningSupport,
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
        context_length: ModelContextLength::new(Some(32_768), Some(32_768), Some(8_192)),
        mode: Some(ModelMode::Chat),
        input_modalities: Some(vec![InputModality::Audio]),
        output_modalities: Some(vec![OutputModality::Text]),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: ["asr_options", "max_tokens", "temperature"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        reasoning: ReasoningSupport::Unsupported,
        reasoning_levels: Vec::new(),
    }
}
