//! Complete canonical model facts for Qwen Audio 3.0 TTS Plus
//! (`qwen/qwen-audio-3.0-tts-plus`).
//!
//! Facts follow the OpenRouter model and endpoint records reverified on 2026-08-24. OpenRouter's
//! zero context value is retained conservatively as an unknown token limit.
//! Qwen-Audio does not support OpenAI-compatible Chat/Responses, so this model remains unrouted
//! until OpenBridge implements the corresponding DashScope-native speech-synthesis operation.

use crate::registry::{
    CanonicalModelTask, ModelConfig, ModelContextLength, SpeechSynthesisModelProfile,
};

/// Stable OpenBridge catalog ID for Qwen Audio 3.0 TTS Plus.
pub(crate) const ID: &str = "qwen/qwen-audio-3.0-tts-plus";

/// Builds the provider-independent speech-synthesis facts for the model.
pub(crate) fn config() -> ModelConfig {
    ModelConfig {
        id: ID.to_owned(),
        name: "Qwen Audio 3.0 TTS Plus".to_owned(),
        description: Some(
            "Higher-quality Qwen text-to-speech model for controllable, expressive speech synthesis."
                .to_owned(),
        ),
        tokenizer: Some("Other".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::SpeechSynthesis(SpeechSynthesisModelProfile {
            context_length: ModelContextLength::new(None, None, None),
            supported_parameters: [
                "max_tokens",
                "presence_penalty",
                "response_format",
                "seed",
                "temperature",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }),
    }
}
