//! Registers Xiaomi MiMo V2.5 Upstream Targets and dual-protocol Upstream APIs.

use std::time::Duration;

use crate::{
    core::AudioCapabilities,
    models::xiaomi,
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::{ProviderInstanceConfig, UpstreamTargetConfig},
};

use super::definition::{ASR_AUDIO, CONTRACT, TTS_AUDIO, VOICE_CLONE_AUDIO, VOICE_DESIGN_AUDIO};

const PROVIDER_INSTANCE_ID: &str = "mimo";

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
            false,
            None,
        ),
        target(
            "mimo-v2-5",
            xiaomi::mimo_v2_5::ID,
            "mimo-v2.5",
            "mimo-primary",
            true,
            None,
        ),
        target(
            "mimo-v2-5-asr",
            xiaomi::mimo_v2_5_asr::ID,
            "mimo-v2.5-asr",
            "mimo-primary",
            false,
            Some(ASR_AUDIO),
        ),
        target(
            "mimo-v2-5-tts",
            xiaomi::mimo_v2_5_tts::ID,
            "mimo-v2.5-tts",
            "mimo-primary",
            false,
            Some(TTS_AUDIO),
        ),
        target(
            "mimo-v2-5-tts-voicedesign",
            xiaomi::mimo_v2_5_tts_voicedesign::ID,
            "mimo-v2.5-tts-voicedesign",
            "mimo-primary",
            false,
            Some(VOICE_DESIGN_AUDIO),
        ),
        target(
            "mimo-v2-5-tts-voiceclone",
            xiaomi::mimo_v2_5_tts_voiceclone::ID,
            "mimo-v2.5-tts-voiceclone",
            "mimo-primary",
            false,
            Some(VOICE_CLONE_AUDIO),
        ),
    ]
}

/// Builds a Chat/Responses target for a MiMo V2.5 model.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
    image_input: bool,
    audio: Option<AudioCapabilities>,
) -> UpstreamTargetConfig {
    // Narrow the Provider ceiling because current MiMo evidence limits image understanding to V2.5.
    let mut capabilities = *CONTRACT.capabilities();
    if !image_input {
        capabilities.chat_completions.image_input = None;
        capabilities.responses.image_input = None;
    }

    // Narrow the Provider audio ceiling and unrelated generation features to the model-specific Chat task.
    capabilities.chat_completions.audio = audio;
    if audio.is_some() {
        // Dedicated audio models ignore function tools, so their fixed interface must fail closed before egress.
        capabilities.chat_completions.function_tools = None;
    }

    let mut upstream_apis = native_upstream_apis(upstream_model, capabilities);
    if audio.is_some() {
        // MiMo audio models expose only Chat Native; do not create a Responses or Bridge candidate.
        upstream_apis.truncate(1);
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
