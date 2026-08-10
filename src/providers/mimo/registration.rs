//! Registers Xiaomi MiMo V2.5 dual-protocol text/image and Chat-only audio target surfaces.

use std::time::Duration;

use crate::{
    core::{
        ExecutableAudioProfile, ExecutableResponsesState, ReasoningOutput, ResponsesAffinity,
        StorageSupport,
    },
    models::xiaomi,
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::{ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamTargetConfig},
};

use super::{
    DEFINITION,
    definition::{ASR_AUDIO, TTS_AUDIO, VOICE_CLONE_AUDIO, VOICE_DESIGN_AUDIO},
};

const PROVIDER_INSTANCE_ID: &str = "mimo";

/// Closed model-specific operation and modality profile for a MiMo target.
#[derive(Clone, Copy)]
enum MimoTargetProfile {
    /// Text-only Chat and Responses operations.
    TextOnly,
    /// Image-capable Chat and Responses operations.
    ImageUnderstanding,
    /// One Chat-only audio task.
    Audio(ExecutableAudioProfile),
}

/// Builds the trusted MiMo API deployment used by the checked-in targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::MiMo,
        base_url: "https://api.xiaomimimo.com".to_owned(),
    }
}

/// Builds the fixed upstream targets for MiMo text, multimodal, ASR, and TTS-family models.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        target(
            "mimo-v2-5-pro",
            xiaomi::mimo_v2_5_pro::ID,
            "mimo-v2.5-pro",
            "mimo-primary",
            MimoTargetProfile::TextOnly,
        ),
        target(
            "mimo-v2-5",
            xiaomi::mimo_v2_5::ID,
            "mimo-v2.5",
            "mimo-primary",
            MimoTargetProfile::ImageUnderstanding,
        ),
        target(
            "mimo-v2-5-asr",
            xiaomi::mimo_v2_5_asr::ID,
            "mimo-v2.5-asr",
            "mimo-primary",
            MimoTargetProfile::Audio(ASR_AUDIO),
        ),
        target(
            "mimo-v2-5-tts",
            xiaomi::mimo_v2_5_tts::ID,
            "mimo-v2.5-tts",
            "mimo-primary",
            MimoTargetProfile::Audio(TTS_AUDIO),
        ),
        target(
            "mimo-v2-5-tts-voicedesign",
            xiaomi::mimo_v2_5_tts_voicedesign::ID,
            "mimo-v2.5-tts-voicedesign",
            "mimo-primary",
            MimoTargetProfile::Audio(VOICE_DESIGN_AUDIO),
        ),
        target(
            "mimo-v2-5-tts-voiceclone",
            xiaomi::mimo_v2_5_tts_voiceclone::ID,
            "mimo-v2.5-tts-voiceclone",
            "mimo-primary",
            MimoTargetProfile::Audio(VOICE_CLONE_AUDIO),
        ),
    ]
}

/// Builds the closed operation surface for one MiMo V2.5 model profile.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
    profile: MimoTargetProfile,
) -> UpstreamTargetConfig {
    // Resolve the Chat ceiling required by every MiMo target.
    let chat_ceiling = DEFINITION
        .contract()
        .capabilities()
        .chat_completions
        .expect("MiMo targets require Chat Completions capabilities");
    let mut chat_capabilities = chat_ceiling.to_executable(match profile {
        MimoTargetProfile::TextOnly | MimoTargetProfile::ImageUnderstanding => None,
        MimoTargetProfile::Audio(audio) => Some(audio),
    });

    // Narrow modalities and operation presence according to the closed model-specific profile.
    let responses_capabilities = match profile {
        MimoTargetProfile::TextOnly | MimoTargetProfile::ImageUnderstanding => {
            let mut responses_capabilities = DEFINITION
                .contract()
                .capabilities()
                .responses
                .expect("MiMo text targets require Responses capabilities")
                .to_executable(ExecutableResponsesState::new(
                    StorageSupport::Unsupported,
                    ResponsesAffinity::TargetBound,
                ));
            if matches!(profile, MimoTargetProfile::TextOnly) {
                chat_capabilities.image_input = None;
                chat_capabilities.function_tools =
                    chat_capabilities.function_tools.map(|mut profile| {
                        profile.parallel_calls = false;
                        profile
                    });
                responses_capabilities.image_input = None;
                responses_capabilities.function_tools =
                    responses_capabilities.function_tools.map(|mut profile| {
                        profile.parallel_calls = false;
                        profile
                    });
                responses_capabilities.include = &[];
            }
            Some(responses_capabilities)
        }
        MimoTargetProfile::Audio(_) => {
            chat_capabilities.image_input = None;
            chat_capabilities.reasoning_output = ReasoningOutput::Unknown;
            chat_capabilities.function_tools = None;
            chat_capabilities.structured_outputs = None;
            None
        }
    };

    // Build only the operations selected by the typed target profile.
    let mut upstream_apis =
        native_upstream_apis(upstream_model, chat_capabilities, responses_capabilities);
    match profile {
        MimoTargetProfile::TextOnly | MimoTargetProfile::ImageUnderstanding => {
            // Current MiMo Responses rejects top_logprobs even though the Chat API accepts it.
            upstream_apis
                .iter_mut()
                .find(|api| matches!(api.capabilities, UpstreamApiCapabilities::Responses(_)))
                .expect("MiMo text targets must expose a Responses API")
                .model_rules
                .disabled_parameters = vec!["top_logprobs".to_owned()];
        }
        MimoTargetProfile::Audio(_) => {}
    }

    // Build the immutable target with the model-specific API ceiling.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::MiMo.routing_model_id(canonical_model),
        credential_pool: credential_id.to_owned(),
        quota_scope: Some("mimo-primary".to_owned()),
        fault_domain: Some("mimo-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis,
    }
}
