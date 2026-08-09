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
fn qwen38_max_compiles_as_bailian_dual_native_with_official_reasoning_levels() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Require the canonical profile and both Bailian APIs to carry the documented reasoning shapes.
    let model = registry
        .model("qwen/qwen3.8-max")
        .expect("Qwen3.8 Max canonical model must compile");
    assert_eq!(
        model.reasoning_levels(),
        &[
            ReasoningLevel::Max,
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::Minimal,
            ReasoningLevel::None,
        ]
    );
    let target = registry
        .upstream_target("bailian-qwen3-8-max")
        .expect("Qwen3.8 Max Bailian target must compile");
    let chat = target
        .upstream_api(OperationKind::ChatCompletions)
        .expect("Qwen3.8 Max must expose Bailian Chat");
    let responses = target
        .upstream_api(OperationKind::Responses)
        .expect("Qwen3.8 Max must expose Bailian Responses");
    assert_eq!(chat.upstream_model(), "qwen3.8-max");
    assert_eq!(chat.reasoning_output(), ReasoningOutput::PlainText);
    assert_eq!(responses.upstream_model(), "qwen3.8-max");
    assert_eq!(responses.reasoning_output(), ReasoningOutput::Summary);

    // Publish one Native candidate per downstream protocol without a lossy Bridge fallback.
    let public_model = registry
        .public_model("qwen3.8-max")
        .expect("Qwen3.8 Max must be public");
    assert!(!public_model.routes().is_empty());
    for (protocol, body) in [
        (
            ApiProtocol::ChatCompletions,
            bytes::Bytes::from_static(
                br#"{"model":"qwen3.8-max","messages":[],"reasoning_effort":"none"}"#,
            ),
        ),
        (
            ApiProtocol::Responses,
            bytes::Bytes::from_static(
                br#"{"model":"qwen3.8-max","input":"hello","reasoning":{"effort":"max"}}"#,
            ),
        ),
    ] {
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        let [candidate] = plan.candidates() else {
            panic!("Qwen3.8 Max {protocol:?} must select one candidate");
        };
        assert_eq!(candidate.upstream_operation(), protocol.operation());
        assert!(candidate.bridge().is_none());
        assert_eq!(candidate.upstream_target_id(), "bailian-qwen3-8-max");
    }
}

#[test]
fn qwen36_27b_preserves_confirmed_parameters_and_binary_reasoning() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let model = registry
        .model("qwen/qwen3.6-27b")
        .expect("Qwen3.6 27B canonical model must compile");

    // Keep model-level context separate from the Alibaba endpoint's narrower output ceiling.
    let context = model.context_length();
    assert_eq!(context.context_tokens(), Some(262_144));
    assert_eq!(context.input_tokens(), Some(262_144));
    assert_eq!(context.output_tokens(), Some(65_536));

    // Match the current OpenRouter model-level parameter set without inventing reasoning efforts.
    assert_eq!(
        model.supported_parameters(),
        [
            "frequency_penalty",
            "include_reasoning",
            "logit_bias",
            "logprobs",
            "max_tokens",
            "min_p",
            "presence_penalty",
            "reasoning",
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
    );
    assert_eq!(
        model.reasoning_levels(),
        &[ReasoningLevel::High, ReasoningLevel::None]
    );
}

#[test]
fn deepseek_flash_responses_exposes_only_proven_tool_choice_modes() {
    // Compile the checked-in multi-source interface and inspect its downstream contract.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let info = serde_json::to_value(
        registry
            .public_model("deepseek-v4-flash")
            .expect("DeepSeek Flash Public Model must exist")
            .info(),
    )
    .unwrap();
    assert_eq!(
        info["interfaces"]["responses"]["tools"]["tool_choice_modes"],
        serde_json::json!(["none", "auto"])
    );

    // Admit the two proven modes through every fixed Responses candidate.
    for choice in [serde_json::json!("none"), serde_json::json!("auto")] {
        let body = bytes::Bytes::from(
            serde_json::to_vec(&serde_json::json!({
                "model": "deepseek-v4-flash",
                "input": "Use the synthetic tool only when allowed.",
                "tools": [{"type": "function", "name": "report_result", "parameters": {"type": "object"}}],
                "tool_choice": choice
            }))
            .unwrap(),
        );
        let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
        plan_request(&registry, &profile, body).expect("the proven choice must be plannable");
    }

    // Reject force-call modes before building any upstream request.
    for choice in [
        serde_json::json!("required"),
        serde_json::json!({"type": "function", "name": "report_result"}),
    ] {
        let body = bytes::Bytes::from(
            serde_json::to_vec(&serde_json::json!({
                "model": "deepseek-v4-flash",
                "input": "Call the synthetic tool.",
                "tools": [{"type": "function", "name": "report_result", "parameters": {"type": "object"}}],
                "tool_choice": choice
            }))
            .unwrap(),
        );
        let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
        assert!(matches!(
            plan_request(&registry, &profile, body),
            Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
        ));
    }
}

#[test]
fn deepseek_public_interfaces_expose_json_object_across_fixed_candidates() {
    // Compile the checked-in multi-source DeepSeek interfaces and require their exact shared profile.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let json_object_profile = serde_json::json!({
        "support": "supported",
        "modes": ["json_object"],
        "strict_schema": "unsupported"
    });

    // Keep both public protocols aligned while preserving each model's complete fixed candidate set.
    for model_id in ["deepseek-v4-pro", "deepseek-v4-flash"] {
        let model = registry
            .public_model(model_id)
            .expect("DeepSeek Public Model must exist");
        let info = serde_json::to_value(model.info()).unwrap();
        for protocol in ["chat_completions", "responses"] {
            assert_eq!(
                info["interfaces"][protocol]["structured_outputs"], json_object_profile,
                "{model_id} {protocol}"
            );
        }
    }

    let pro_responses = bytes::Bytes::from_static(
        br#"{"model":"deepseek-v4-pro","input":"Return json like {\"result\":\"ok\"}.","text":{"format":{"type":"json_object"}}}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &pro_responses).unwrap();
    let plan = plan_request(&registry, &profile, pro_responses).unwrap();
    assert_eq!(plan.candidates().len(), 2);
    assert!(
        plan.candidates()
            .iter()
            .all(|candidate| candidate.bridge().is_some())
    );

    let flash_responses = bytes::Bytes::from_static(
        br#"{"model":"deepseek-v4-flash","input":"Return json like {\"result\":\"ok\"}.","text":{"format":{"type":"json_object"}}}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &flash_responses).unwrap();
    let plan = plan_request(&registry, &profile, flash_responses).unwrap();
    assert_eq!(plan.candidates().len(), 2);
    assert!(
        plan.candidates()
            .iter()
            .all(|candidate| candidate.bridge().is_none())
    );

    // Prevent Provider ceilings from leaking this model-specific evidence into unrelated targets.
    for target_id in [
        "openrouter-minimax-m3",
        "bailian-glm-5-2",
        "bailian-qwen3-7-plus",
        "bailian-qwen3-7-max",
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("unrelated target must remain registered");
        for (_, api) in target.upstream_apis() {
            let structured_outputs = match api.capabilities() {
                UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                    capabilities.structured_outputs
                }
                UpstreamApiCapabilities::Responses(capabilities) => capabilities.structured_outputs,
                UpstreamApiCapabilities::Embeddings(_) => None,
            };
            assert!(structured_outputs.is_none(), "{target_id}");
        }
    }
}

#[test]
fn mimo_public_interfaces_expose_only_proven_tool_and_structured_output_profiles() {
    // Compile the checked-in registry and compare each text model's operation-specific profile.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let model_info = |model_id: &str| {
        serde_json::to_value(
            registry
                .public_model(model_id)
                .expect("MiMo Public Model must exist")
                .info(),
        )
        .unwrap()
    };
    let text_tool_modes = serde_json::json!(["auto"]);
    let json_object_profile = serde_json::json!({
        "support": "supported",
        "modes": ["json_object"],
        "strict_schema": "unsupported"
    });

    let v25 = model_info("mimo-v2.5");
    for protocol in ["chat_completions", "responses"] {
        assert_eq!(
            v25["interfaces"][protocol]["tools"]["tool_choice_modes"],
            text_tool_modes
        );
        assert_eq!(
            v25["interfaces"][protocol]["tools"]["parallel_calls"],
            "unsupported"
        );
        assert_eq!(
            v25["interfaces"][protocol]["tools"]["strict_schema"],
            "supported"
        );
        assert_eq!(
            v25["interfaces"][protocol]["structured_outputs"],
            json_object_profile
        );
    }

    let pro = model_info("mimo-v2.5-pro");
    for protocol in ["chat_completions", "responses"] {
        assert_eq!(
            pro["interfaces"][protocol]["tools"]["tool_choice_modes"],
            text_tool_modes
        );
        assert_eq!(
            pro["interfaces"][protocol]["tools"]["parallel_calls"],
            "unsupported"
        );
        assert_eq!(
            pro["interfaces"][protocol]["tools"]["strict_schema"],
            "supported"
        );
        assert_eq!(
            pro["interfaces"][protocol]["structured_outputs"],
            json_object_profile
        );
    }

    // Keep structured text constraints out of every task-specific audio interface.
    for model_id in [
        "mimo-v2.5-asr",
        "mimo-v2.5-tts",
        "mimo-v2.5-tts-voicedesign",
        "mimo-v2.5-tts-voiceclone",
    ] {
        let info = model_info(model_id);
        assert_eq!(
            info["interfaces"]["chat_completions"]["structured_outputs"]["support"], "unsupported",
            "{model_id}"
        );
        assert_eq!(
            info["interfaces"]["chat_completions"]["structured_outputs"]["modes"],
            serde_json::json!([]),
            "{model_id}"
        );
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
