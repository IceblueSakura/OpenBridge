//! Verifies checked-in Provider boundaries through registry compilation and request planning.

use super::*;

#[test]
fn checked_in_targets_stay_within_provider_and_credential_boundaries() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Relate every checked-in Target to its Provider credential and protocol ceilings.
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        let pool = registry
            .credential_pool(target.credential_pool_id())
            .expect("Target credential pool must resolve");
        let contract = target.kind().contract();
        assert_eq!(pool.provider(), target.kind(), "{target_id}");
        assert!(
            contract.credential_kinds().contains(&pool.kind()),
            "{target_id} uses an unsupported credential kind"
        );
        assert_eq!(target.endpoint_base().scheme(), "https", "{target_id}");

        for (operation, _) in target.upstream_apis() {
            let supported = match operation {
                OperationKind::ChatCompletions => contract.capabilities().chat_completions.enabled,
                OperationKind::Responses => contract.capabilities().responses.enabled,
                OperationKind::EmbeddingsCreate => contract.capabilities().embeddings.enabled,
            };
            assert!(
                supported,
                "{target_id} exceeds its Provider contract with {operation}"
            );
        }
    }
}

#[test]
fn checked_in_fallback_chains_follow_provider_priority_and_protocol_boundaries() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let cases: [(&str, ApiProtocol, &[u8], &[(ProviderKind, bool)]); 7] = [
        (
            "Kimi Responses Bridge",
            ApiProtocol::Responses,
            br#"{"model":"kimi-k3","input":"hello","stream":true}"#,
            &[(ProviderKind::KimiCn, true)],
        ),
        (
            "MiniMax Chat fallbacks",
            ApiProtocol::ChatCompletions,
            br#"{"model":"minimax-m3","messages":[],"reasoning_effort":"high"}"#,
            &[
                (ProviderKind::OpenRouter, false),
                (ProviderKind::Nvidia, false),
            ],
        ),
        (
            "DeepSeek Flash Chat fallbacks",
            ApiProtocol::ChatCompletions,
            br#"{"model":"deepseek-v4-flash","messages":[],"reasoning_effort":"high"}"#,
            &[
                (ProviderKind::DeepSeek, false),
                (ProviderKind::OpenRouter, false),
                (ProviderKind::Bailian, false),
            ],
        ),
        (
            "Qwen Chat Native",
            ApiProtocol::ChatCompletions,
            br#"{"model":"qwen3.7-max","messages":[]}"#,
            &[(ProviderKind::Bailian, false)],
        ),
        (
            "Qwen Responses Native",
            ApiProtocol::Responses,
            br#"{"model":"qwen3.7-max","input":"hello"}"#,
            &[(ProviderKind::Bailian, false)],
        ),
        (
            "MiMo Chat Native",
            ApiProtocol::ChatCompletions,
            br#"{"model":"mimo-v2.5","messages":[]}"#,
            &[(ProviderKind::MiMo, false)],
        ),
        (
            "MiMo Responses Native",
            ApiProtocol::Responses,
            br#"{"model":"mimo-v2.5","input":"hello"}"#,
            &[(ProviderKind::MiMo, false)],
        ),
    ];

    // Plan representative requests and compare observable Provider order instead of copied Route IDs.
    for (case, protocol, request, expected) in cases {
        let body = bytes::Bytes::copy_from_slice(request);
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        let actual = plan
            .candidates()
            .iter()
            .map(|candidate| {
                let target = registry
                    .upstream_target(candidate.upstream_target_id())
                    .expect("planned Target must resolve");
                (target.kind(), candidate.bridge().is_some())
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{case}");
    }
}

#[test]
fn every_advertised_reasoning_level_is_plannable() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let mut checked_interfaces = 0;

    // Derive requests from the published contract so advertised levels and preflight cannot drift apart.
    for public_model in registry.public_models() {
        let model_id = public_model.standard().id();
        let info = serde_json::to_value(public_model.info()).unwrap();
        for (protocol, interface_name) in [
            (ApiProtocol::ChatCompletions, "chat_completions"),
            (ApiProtocol::Responses, "responses"),
        ] {
            let Some(levels) = info["interfaces"][interface_name]["reasoning"]["levels"].as_array()
            else {
                continue;
            };
            if levels.is_empty() {
                continue;
            }
            checked_interfaces += 1;

            for level in levels {
                let level = level
                    .as_str()
                    .expect("reasoning level must serialize as text");
                let request = match protocol {
                    ApiProtocol::ChatCompletions => format!(
                        r#"{{"model":"{model_id}","messages":[],"reasoning_effort":"{level}"}}"#
                    ),
                    ApiProtocol::Responses => format!(
                        r#"{{"model":"{model_id}","input":"hello","reasoning":{{"effort":"{level}"}}}}"#
                    ),
                };
                let body = bytes::Bytes::from(request);
                let profile = analyze_request(protocol, &body).unwrap();
                let plan = plan_request(&registry, &profile, body).unwrap_or_else(|error| {
                    panic!("{model_id} {interface_name} {level}: {error:?}")
                });
                assert!(!plan.candidates().is_empty(), "{model_id} {interface_name}");
            }
        }
    }
    assert!(checked_interfaces > 0);
}

#[test]
fn deepseek_v4_pro_responses_high_preserves_both_fallbacks() {
    // Compile the production registry and require the weaker Bailian target to expose readable reasoning.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let bailian = registry
        .upstream_target("bailian-deepseek-v4-pro")
        .unwrap()
        .upstream_api(OperationKind::ChatCompletions)
        .unwrap();
    assert_eq!(bailian.reasoning_output(), ReasoningOutput::PlainText);

    // Verify the fixed Responses contract accepts high without dropping either Chat fallback.
    let body = bytes::Bytes::from_static(
        br#"{"model":"deepseek-v4-pro","input":"hello","reasoning":{"effort":"high"}}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(
        plan.candidates()
            .iter()
            .map(|candidate| {
                registry
                    .upstream_target(candidate.upstream_target_id())
                    .unwrap()
                    .kind()
            })
            .collect::<Vec<_>>(),
        [ProviderKind::DeepSeek, ProviderKind::Bailian]
    );
    assert!(
        plan.candidates()
            .iter()
            .all(|candidate| candidate.bridge().is_some())
    );
}

#[test]
fn mimo_audio_targets_do_not_inherit_text_reasoning_evidence() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Keep audio reasoning fail-closed and preserve the Chat-only protocol surface.
    for target_id in [
        "mimo-v2-5-asr",
        "mimo-v2-5-tts",
        "mimo-v2-5-tts-voicedesign",
        "mimo-v2-5-tts-voiceclone",
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("MiMo audio target should compile");
        assert_eq!(
            target
                .upstream_api(OperationKind::ChatCompletions)
                .expect("MiMo audio target should expose Chat")
                .reasoning_output(),
            ReasoningOutput::Unknown
        );
        assert!(target.upstream_api(OperationKind::Responses).is_none());
    }
}

#[test]
fn mimo_v25_native_responses_accepts_prior_reasoning_items() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Keep an existing reasoning item on the Native route without introducing a lossy Bridge candidate.
    let body = bytes::Bytes::from_static(
        br#"{"model":"mimo-v2.5","input":[{"type":"reasoning","id":"rs_1","summary":[{"type":"summary_text","text":"prior"}]}]}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    let [candidate] = plan.candidates() else {
        panic!("MiMo reasoning replay must select one Native candidate");
    };
    assert_eq!(candidate.upstream_operation(), OperationKind::Responses);
    assert!(candidate.bridge().is_none());
    assert_eq!(
        registry
            .upstream_target(candidate.upstream_target_id())
            .unwrap()
            .kind(),
        ProviderKind::MiMo
    );
}
