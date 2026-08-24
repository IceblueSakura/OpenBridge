//! Aggregates canonical model facts for the Qwen family.

use crate::registry::ModelConfig;

pub(crate) mod qwen3_7_max;
pub(crate) mod qwen3_7_plus;
pub(crate) mod qwen3_7_text_embedding;
pub(crate) mod qwen3_8_27b;
pub(crate) mod qwen3_8_max;
pub(crate) mod qwen_audio_3_0_asr_flash;
pub(crate) mod qwen_audio_3_0_realtime_flash;
pub(crate) mod qwen_audio_3_0_realtime_plus;
pub(crate) mod qwen_audio_3_0_tts_plus;
pub(crate) mod qwen_image_3_0;
pub(crate) mod qwen_image_3_0_pro;

/// Returns all Qwen model facts compiled into the binary.
pub(crate) fn configs() -> Vec<ModelConfig> {
    vec![
        qwen3_7_max::config(),
        qwen3_7_plus::config(),
        qwen3_7_text_embedding::config(),
        qwen3_8_27b::config(),
        qwen3_8_max::config(),
        qwen_image_3_0::config(),
        qwen_image_3_0_pro::config(),
        qwen_audio_3_0_asr_flash::config(),
        qwen_audio_3_0_realtime_flash::config(),
        qwen_audio_3_0_realtime_plus::config(),
        qwen_audio_3_0_tts_plus::config(),
    ]
}

#[cfg(test)]
mod tests {
    use crate::registry::{
        CanonicalModelTask, InputModality, ModelConfig, ModelContextLength, OutputModality,
        ReasoningLevel, ReasoningProfile,
    };

    fn model(id: &str) -> ModelConfig {
        super::configs()
            .into_iter()
            .find(|model| model.id == id)
            .unwrap_or_else(|| panic!("{id} must be present in the canonical catalog"))
    }

    #[test]
    fn configs_exclude_removed_qwen_models() {
        let ids = super::configs()
            .into_iter()
            .map(|model| model.id)
            .collect::<Vec<_>>();
        assert!(
            !ids.iter()
                .any(|id| id == "qwen/qwen3.5-livetranslate-flash-realtime")
        );
        assert!(!ids.iter().any(|id| id == "qwen/qwen3.6-27b"));
    }

    #[test]
    fn configs_include_qwen_audio_3_0_realtime_flash_official_facts() {
        let model = model("qwen/qwen-audio-3.0-realtime-flash");
        assert_eq!(model.name, "Qwen Audio 3.0 Realtime Flash");
        assert_eq!(model.tokenizer, None);

        let CanonicalModelTask::Generation(profile) = model.task else {
            panic!("Qwen Audio 3.0 Realtime Flash must be a generation model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(None, None, None)
        );
        assert_eq!(
            profile.input_modalities,
            Some(vec![InputModality::Text, InputModality::Audio])
        );
        assert_eq!(
            profile.output_modalities,
            Some(vec![OutputModality::Text, OutputModality::Audio])
        );
        assert!(profile.supported_parameters.is_empty());
        assert_eq!(profile.reasoning, ReasoningProfile::Unsupported);
    }

    #[test]
    fn configs_include_qwen_audio_3_0_asr_flash_official_facts() {
        let model = model("qwen/qwen-audio-3.0-asr-flash");
        assert_eq!(model.name, "Qwen Audio 3.0 ASR Flash");
        assert_eq!(model.tokenizer, None);

        let CanonicalModelTask::SpeechRecognition(profile) = model.task else {
            panic!("Qwen Audio 3.0 ASR Flash must be a speech-recognition model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(None, None, None)
        );
        assert!(profile.supported_parameters.is_empty());
    }

    #[test]
    fn configs_include_qwen_audio_3_0_realtime_plus_official_facts() {
        let model = model("qwen/qwen-audio-3.0-realtime-plus");
        assert_eq!(model.name, "Qwen Audio 3.0 Realtime Plus");
        assert_eq!(model.tokenizer, None);

        let CanonicalModelTask::Generation(profile) = model.task else {
            panic!("Qwen Audio 3.0 Realtime Plus must be a generation model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(None, None, None)
        );
        assert_eq!(
            profile.input_modalities,
            Some(vec![InputModality::Text, InputModality::Audio])
        );
        assert_eq!(
            profile.output_modalities,
            Some(vec![OutputModality::Text, OutputModality::Audio])
        );
        assert!(profile.supported_parameters.is_empty());
        assert_eq!(profile.reasoning, ReasoningProfile::Unsupported);
    }

    #[test]
    fn configs_include_qwen_audio_3_0_tts_plus_openrouter_facts() {
        let model = model("qwen/qwen-audio-3.0-tts-plus");
        assert_eq!(model.name, "Qwen Audio 3.0 TTS Plus");
        assert_eq!(model.tokenizer.as_deref(), Some("Other"));

        let CanonicalModelTask::SpeechSynthesis(profile) = model.task else {
            panic!("Qwen Audio 3.0 TTS Plus must be a speech-synthesis model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(None, None, None)
        );
        assert_eq!(
            profile.supported_parameters,
            [
                "max_tokens",
                "presence_penalty",
                "response_format",
                "seed",
                "temperature",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn configs_include_qwen3_8_27b_openrouter_facts() {
        let model = model("qwen/qwen3.8-27b");
        assert_eq!(model.name, "Qwen3.8 27B");
        assert_eq!(model.tokenizer.as_deref(), Some("Qwen"));

        let CanonicalModelTask::Generation(profile) = model.task else {
            panic!("Qwen3.8 27B must be a generation model");
        };
        assert_eq!(
            profile.context_length,
            ModelContextLength::new(Some(1_000_000), Some(1_000_000), Some(131_072))
        );
        assert_eq!(
            profile.input_modalities,
            Some(vec![
                InputModality::Text,
                InputModality::Image,
                InputModality::Video,
            ])
        );
        assert_eq!(profile.output_modalities, Some(vec![OutputModality::Text]));
        assert_eq!(
            profile.supported_parameters,
            [
                "frequency_penalty",
                "include_reasoning",
                "logit_bias",
                "logprobs",
                "max_tokens",
                "presence_penalty",
                "repetition_penalty",
                "response_format",
                "seed",
                "stop",
                "structured_outputs",
                "temperature",
                "tool_choice",
                "tools",
                "top_k",
                "top_logprobs",
                "top_p",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        );
        assert_eq!(
            profile.reasoning.levels(),
            [
                ReasoningLevel::XHigh,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]
        );
    }
}
