//! Verifies checked-in bootstrap examples, credential pools, and compiled registry consistency.

use super::*;
use std::collections::BTreeMap;

use openbridge::{
    core::{
        AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputLimits, AudioInputSource,
        AudioUnderstandingProfile, ExecutableAudioProfile, SpeechRecognitionProfile,
    },
    registry::{
        CanonicalModelTask, CanonicalTaskKind, InputModality, ModelLifecycle, OutputModality,
        PublicModelConfig, RegistryError, UpstreamTargetConfig,
    },
};

const ONLY_ZH: &[AsrLanguage] = &[AsrLanguage::Zh];
const ONLY_EN: &[AsrLanguage] = &[AsrLanguage::En];

#[test]
fn canonical_catalog_assigns_every_model_to_one_expected_task() {
    use CanonicalTaskKind::{
        Embedding, Generation, SpeechRecognition, SpeechSynthesis, VoiceClone, VoiceDesign,
    };

    // Declare the complete canonical catalog so additions and task changes require an explicit review.
    let expected = BTreeMap::from([
        ("chatgpt/gpt-5.3-codex-spark", Generation),
        ("chatgpt/gpt-5.5", Generation),
        ("chatgpt/gpt-5.6-luna", Generation),
        ("chatgpt/gpt-5.6-sol", Generation),
        ("chatgpt/gpt-5.6-terra", Generation),
        ("deepseek/deepseek-v4-flash", Generation),
        ("deepseek/deepseek-v4-pro", Generation),
        ("meituan/longcat-2.0", Generation),
        ("minimax/minimax-m3", Generation),
        ("moonshotai/kimi-k3", Generation),
        ("openai/gpt-5.5", Generation),
        ("openai/gpt-5.6-luna", Generation),
        ("openai/gpt-5.6-sol", Generation),
        ("openai/gpt-5.6-terra", Generation),
        ("openai/text-embedding-3-small", Embedding),
        ("qwen/qwen-audio-3.0-asr-flash", SpeechRecognition),
        ("qwen/qwen-image-3.0", Generation),
        ("qwen/qwen-image-3.0-pro", Generation),
        ("qwen/qwen3.5-livetranslate-flash-realtime", Generation),
        ("qwen/qwen3.6-27b", Generation),
        ("qwen/qwen3.7-max", Generation),
        ("qwen/qwen3.7-plus", Generation),
        ("qwen/qwen3.7-text-embedding", Embedding),
        ("qwen/qwen3.8-max", Generation),
        ("xiaomi/mimo-v2.5", Generation),
        ("xiaomi/mimo-v2.5-asr", SpeechRecognition),
        ("xiaomi/mimo-v2.5-pro", Generation),
        ("xiaomi/mimo-v2.5-tts", SpeechSynthesis),
        ("xiaomi/mimo-v2.5-tts-voiceclone", VoiceClone),
        ("xiaomi/mimo-v2.5-tts-voicedesign", VoiceDesign),
        ("z-ai/glm-5.2", Generation),
    ]);

    // Compare both identity and task payload variant for all 31 catalog entries.
    let actual = compiled_config()
        .models
        .into_iter()
        .map(|model| (model.id, model.task.kind()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual.len(), 31);
    assert_eq!(
        actual.values().fold(BTreeMap::new(), |mut counts, task| {
            *counts.entry(*task).or_insert(0_usize) += 1;
            counts
        }),
        BTreeMap::from([
            (Generation, 24),
            (Embedding, 2),
            (SpeechRecognition, 2),
            (SpeechSynthesis, 1),
            (VoiceDesign, 1),
            (VoiceClone, 1),
        ])
    );
    assert_eq!(
        actual,
        expected
            .into_iter()
            .map(|(id, task)| (id.to_owned(), task))
            .collect()
    );
}

#[test]
fn checked_in_examples_compile_into_a_closed_runtime_registry() {
    // Parse the active and example bootstrap documents as one maintained process policy.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml"))
        .expect("checked-in bootstrap must remain valid");
    let template = parse_bootstrap_config(include_str!("../../config/bootstrap.example.toml"))
        .expect("checked-in bootstrap template must remain valid");
    assert_eq!(template, bootstrap);
    assert!(bootstrap.listen().ip().is_loopback());
    let registry =
        build_compiled_registry(bootstrap).expect("compiled registry must remain internally valid");
    let users = UserConfigPath::new("config/users.example.toml")
        .load()
        .expect("checked-in user example must remain valid");
    assert!(users.users().users().next().is_some());

    // Resolve every published Route through a trusted Target and one declared Upstream API.
    let mut public_model_count = 0;
    for public_model in registry.public_models() {
        public_model_count += 1;
        assert!(
            !public_model.routes().is_empty(),
            "{} has no executable Route",
            public_model.standard().id()
        );
        for route_id in public_model.routes() {
            let route = registry
                .route(route_id)
                .expect("published Route must resolve");
            let target = registry
                .upstream_target(route.upstream_target())
                .expect("published Route Target must resolve");
            assert!(target.enabled(), "{} is not selectable", target.id());
            assert!(
                target.upstream_api(route.upstream_operation()).is_some(),
                "{route_id} references an unavailable Upstream API"
            );
        }
    }
    assert!(public_model_count > 0);

    // Keep every compiled Target on HTTPS and bound to a declared credential pool.
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        assert_eq!(target.endpoint_base().scheme(), "https", "{target_id}");
        assert!(
            registry
                .credential_pool(target.credential_pool_id())
                .is_some(),
            "{target_id} has no credential pool"
        );
    }
}

#[test]
fn compiled_provider_credential_pools_are_shared_and_match_the_private_toml_example() {
    // Build the registry and load only API-key pools from a TOML template with no real values.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let pool_ids = registry
        .credential_pool_ids()
        .filter(|pool_id| {
            registry
                .credential_pool(pool_id)
                .is_some_and(|pool| pool.kind() == CredentialKind::ApiKey)
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let credentials = UpstreamCredentialConfiguration::from_toml(include_str!(
        "../../config/upstream-credentials.example.toml"
    ))
    .unwrap()
    .into_builder_for(&registry, pool_ids.iter().map(String::as_str))
    .unwrap()
    .build();

    // Verify that each API-key target retrieves the template credential by Provider and pool.
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        let pool = registry
            .credential_pool(target.credential_pool_id())
            .unwrap();
        if pool.kind() != CredentialKind::ApiKey {
            continue;
        }
        assert!(
            credentials
                .upstream_pool(target.kind(), target.credential_pool_id(), pool.kind(),)
                .is_ok()
        );
    }

    // Keep the ChatGPT OAuth pool outside the immutable API-key credential snapshot.
    assert!(
        credentials
            .upstream_pool(
                ProviderKind::ChatGpt,
                "chatgpt-codex",
                CredentialKind::OAuth2BearerAccessToken,
            )
            .is_err()
    );
}

#[test]
fn canonical_audio_task_rejects_missing_and_different_executable_profiles() {
    let base = compiled_config();
    let tts_profile = base
        .upstream_targets
        .iter()
        .find(|target| target.id == "mimo-v2-5-tts")
        .and_then(|target| target.upstream_apis.first())
        .and_then(|api| match api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities.audio,
            UpstreamApiCapabilities::Responses(_) | UpstreamApiCapabilities::Embeddings(_) => None,
        })
        .expect("MiMo TTS executable profile must exist");

    // Reject both a missing specialist profile and a different profile that remains inside the Provider ceiling.
    for audio in [None, Some(tts_profile)] {
        let mut definition = base.clone();
        let target = definition
            .upstream_targets
            .iter_mut()
            .find(|target| target.id == "mimo-v2-5-asr")
            .expect("MiMo ASR target must exist");
        let UpstreamApiCapabilities::ChatCompletions(capabilities) =
            &mut target.upstream_apis[0].capabilities
        else {
            panic!("MiMo ASR target must expose Chat Completions");
        };
        capabilities.audio = audio;

        let bootstrap =
            parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
        assert!(matches!(
            build_registry(bootstrap, definition),
            Err(RegistryError::UpstreamApiModelTaskMismatch {
                upstream_target,
                upstream_operation: OperationKind::ChatCompletions,
                canonical_model,
            }) if upstream_target == "mimo-v2-5-asr"
                && canonical_model == "xiaomi/mimo-v2.5-asr"
        ));
    }
}

#[test]
fn provider_audio_ceiling_rejects_an_oversized_executable_profile_before_task_matching() {
    let mut definition = compiled_config();
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "mimo-v2-5-asr")
        .expect("MiMo ASR target must exist");
    let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut target.upstream_apis[0].capabilities
    else {
        panic!("MiMo ASR target must expose Chat Completions");
    };

    // Keep the ASR variant correct while exceeding the Provider input-parts ceiling by one.
    capabilities.audio = Some(ExecutableAudioProfile::SpeechRecognition(
        SpeechRecognitionProfile::new(
            AudioInputCapabilities::new(
                &[AudioInputSource::DataUrl],
                &[AudioFormat::Wav],
                AudioInputLimits::new(65, 0, 1, 1, 1, 1),
            ),
            ONLY_ZH,
        ),
    ));

    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    assert!(matches!(
        build_registry(bootstrap, definition),
        Err(RegistryError::CapabilityElevation {
            upstream_target,
            upstream_operation: OperationKind::ChatCompletions,
        }) if upstream_target == "mimo-v2-5-asr"
    ));
}

#[test]
fn audio_understanding_requires_confirmed_audio_input_and_text_output() {
    // Accept the executable profile only for the complete confirmed Generation modality pair.
    let mut valid = compiled_config();
    configure_audio_understanding_target(&mut valid);
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    build_registry(bootstrap, valid).expect("confirmed Audio-to-Text Generation must compile");

    // Reject unknown or explicitly incomplete input and output modality evidence with one stable error.
    for (input, output) in [
        (None, Some(vec![OutputModality::Text])),
        (
            Some(vec![InputModality::Text]),
            Some(vec![OutputModality::Text]),
        ),
        (Some(vec![InputModality::Audio]), None),
        (
            Some(vec![InputModality::Audio]),
            Some(vec![OutputModality::Audio]),
        ),
    ] {
        let mut definition = compiled_config();
        configure_audio_understanding_target(&mut definition);
        let model = definition
            .models
            .iter_mut()
            .find(|model| model.id == "xiaomi/mimo-v2.5")
            .expect("MiMo generation canonical Model must exist");
        let CanonicalModelTask::Generation(profile) = &mut model.task else {
            panic!("MiMo-V2.5 must remain a Generation canonical task");
        };
        profile.input_modalities = input;
        profile.output_modalities = output;

        let bootstrap =
            parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
        assert!(matches!(
            build_registry(bootstrap, definition),
            Err(RegistryError::UpstreamApiModelTaskMismatch {
                upstream_target,
                upstream_operation: OperationKind::ChatCompletions,
                canonical_model,
            }) if upstream_target == "mimo-v2-5-asr"
                && canonical_model == "xiaomi/mimo-v2.5"
        ));
    }
}

#[test]
fn public_model_rejects_cross_operation_canonical_task_mixture() {
    let mut definition = compiled_config();
    let generation_route = definition
        .routes
        .iter()
        .find(|route| {
            route.upstream_target == "mimo-v2-5"
                && route.downstream_operation == OperationKind::ChatCompletions
        })
        .expect("MiMo generation Chat route must exist")
        .id
        .clone();
    let embedding_route = definition
        .routes
        .iter()
        .find(|route| route.downstream_operation == OperationKind::EmbeddingsCreate)
        .expect("one built-in Embeddings route must exist")
        .id
        .clone();
    definition.public_models.push(PublicModelConfig {
        id: "mixed-canonical-task".to_owned(),
        created: 1_785_715_200,
        display_name: "Mixed canonical task".to_owned(),
        description: None,
        lifecycle: ModelLifecycle::active(),
        routes: vec![generation_route, embedding_route],
    });

    // Reject the Public Model before per-operation interface aggregation can hide the task mixture.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    assert!(matches!(
        build_registry(bootstrap, definition),
        Err(RegistryError::PublicModelTaskMismatch { public_model })
            if public_model == "mixed-canonical-task"
    ));
}

#[test]
fn public_model_rejects_empty_same_variant_audio_profile_intersection() {
    let mut definition = compiled_config();
    let target_index = definition
        .upstream_targets
        .iter()
        .position(|target| target.id == "mimo-v2-5-asr")
        .expect("MiMo ASR target must exist");
    narrow_asr_languages(&mut definition.upstream_targets[target_index], ONLY_ZH);

    // Clone the same canonical ASR target with a disjoint, still Provider-valid language subset.
    let mut disjoint_target = definition.upstream_targets[target_index].clone();
    disjoint_target.id = "mimo-v2-5-asr-disjoint".to_owned();
    narrow_asr_languages(&mut disjoint_target, ONLY_EN);
    definition.upstream_targets.push(disjoint_target);

    // Bind both same-task candidates to one Chat interface so their language sets must intersect.
    let original_route = definition
        .routes
        .iter()
        .find(|route| {
            route.upstream_target == "mimo-v2-5-asr"
                && route.downstream_operation == OperationKind::ChatCompletions
        })
        .expect("MiMo ASR Chat route must exist")
        .clone();
    let mut disjoint_route = original_route.clone();
    disjoint_route.id = "mimo-v2-5-asr-disjoint-chat".to_owned();
    disjoint_route.upstream_target = "mimo-v2-5-asr-disjoint".to_owned();
    definition.routes.push(disjoint_route.clone());
    definition.public_models.push(PublicModelConfig {
        id: "empty-asr-language-intersection".to_owned(),
        created: 1_785_715_200,
        display_name: "Empty ASR language intersection".to_owned(),
        description: None,
        lifecycle: ModelLifecycle::active(),
        routes: vec![original_route.id, disjoint_route.id],
    });

    // Report the stable operation-scoped profile error rather than erasing the audio interface.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    assert!(matches!(
        build_registry(bootstrap, definition),
        Err(RegistryError::PublicModelInterfaceProfileMismatch {
            public_model,
            downstream_operation: OperationKind::ChatCompletions,
        }) if public_model == "empty-asr-language-intersection"
    ));
}

/// Replaces one executable ASR language payload while preserving every other Target fact.
fn narrow_asr_languages(target: &mut UpstreamTargetConfig, languages: &'static [AsrLanguage]) {
    let capabilities = target
        .upstream_apis
        .iter_mut()
        .find_map(|api| match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => Some(capabilities),
            UpstreamApiCapabilities::Responses(_) | UpstreamApiCapabilities::Embeddings(_) => None,
        })
        .expect("MiMo ASR target must expose Chat Completions");
    let Some(ExecutableAudioProfile::SpeechRecognition(profile)) = capabilities.audio else {
        panic!("MiMo ASR target must expose a speech-recognition profile");
    };
    capabilities.audio = Some(ExecutableAudioProfile::SpeechRecognition(
        SpeechRecognitionProfile::new(profile.input(), languages),
    ));
}

/// Converts the synthetic MiMo ASR target into a Provider-valid Generation audio-understanding target.
fn configure_audio_understanding_target(definition: &mut openbridge::registry::RegistryConfig) {
    // Rebind the Target identity to the canonical Generation Model.
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "mimo-v2-5-asr")
        .expect("MiMo ASR target must exist");
    target.canonical_model = "xiaomi/mimo-v2.5".to_owned();
    target.provider_model = "mimo/mimo-v2.5".to_owned();

    // Reuse the checked ASR input payload inside the Provider's audio-understanding ceiling.
    let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut target.upstream_apis[0].capabilities
    else {
        panic!("MiMo ASR target must expose Chat Completions");
    };
    let Some(ExecutableAudioProfile::SpeechRecognition(profile)) = capabilities.audio else {
        panic!("MiMo ASR target must start with a speech-recognition profile");
    };
    capabilities.audio = Some(ExecutableAudioProfile::AudioUnderstanding(
        AudioUnderstandingProfile::new(profile.input()),
    ));
}
