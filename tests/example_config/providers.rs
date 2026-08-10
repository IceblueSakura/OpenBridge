//! Verifies checked-in Provider boundaries through registry compilation and request planning.

use super::*;

#[test]
fn unverified_bailian_qwen_audio_remains_canonical_without_an_executable_target() {
    let config = compiled_config();

    // Retain model-list identity while refusing to invent an unverified Chat audio profile.
    assert!(
        config
            .models
            .iter()
            .any(|model| model.id == "qwen/qwen-audio-3.0-asr-flash")
    );
    assert!(config.upstream_targets.iter().all(|target| {
        target.id != "bailian-qwen-audio-3-0-asr-flash"
            && target.canonical_model != "qwen/qwen-audio-3.0-asr-flash"
    }));
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
                br#"{"model":"qwen3.8-max","messages":[{"role":"user","content":"hello"}],"reasoning_effort":"none"}"#,
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

    // Keep ordinary parameters free of protocol-specific reasoning aliases.
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
    assert_eq!(model.reasoning_support(), ReasoningSupport::Supported);
    assert_eq!(
        model.reasoning_levels(),
        &[ReasoningLevel::High, ReasoningLevel::None]
    );
}

#[test]
fn sparse_reasoning_normalization_precedes_protocol_bridge() {
    // Compile the checked-in Responses-only Spark route with its sparse positive reasoning set.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let info = serde_json::to_value(
        registry
            .public_model("gpt-5.3-codex-spark")
            .expect("Spark Public Model must compile")
            .info(),
    )
    .unwrap();
    let reasoning = &info["interfaces"]["chat_completions"]["reasoning"];
    assert_eq!(
        reasoning["levels"],
        serde_json::json!(["low", "medium", "high", "xhigh"])
    );
    assert_eq!(
        reasoning["accepted_levels"],
        serde_json::json!(["minimal", "low", "medium", "high", "xhigh", "max"])
    );
    assert_eq!(reasoning["input_policy"], "clamp_positive_floor");

    // Normalize Chat input before its single Bridge converts the canonical field to Responses.
    for (requested, expected) in [("minimal", "low"), ("max", "xhigh")] {
        let body = bytes::Bytes::from(format!(
            r#"{{"model":"gpt-5.3-codex-spark","messages":[{{"role":"user","content":"hello"}}],"reasoning_effort":"{requested}"}}"#
        ));
        let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        let [candidate] = plan.candidates() else {
            panic!("Spark Chat must have exactly one bridged candidate");
        };
        assert!(candidate.bridge().is_some());
        assert_eq!(candidate.request().protocol(), ApiProtocol::Responses);
        let upstream: serde_json::Value =
            serde_json::from_slice(candidate.request().body()).unwrap();
        assert_eq!(upstream["reasoning"]["effort"], expected);
    }
}

#[test]
fn live_confirmed_reasoning_off_levels_compile_into_public_interfaces() {
    // Compile the production registry from the checked-in Provider and model definitions.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Require every live-confirmed off level on both downstream generation interfaces.
    for (model_id, expected_levels) in [
        ("deepseek-v4-pro", &["none", "high", "max"][..]),
        ("deepseek-v4-flash", &["none", "low", "high", "max"][..]),
        ("kimi-k3", &["none", "low", "high", "max"][..]),
        ("glm-5.2", &["none", "high", "xhigh"][..]),
    ] {
        let info = serde_json::to_value(
            registry
                .public_model(model_id)
                .unwrap_or_else(|| panic!("{model_id} Public Model must exist"))
                .info(),
        )
        .unwrap();
        for interface in ["chat_completions", "responses"] {
            assert_eq!(
                info["interfaces"][interface]["reasoning"]["levels"],
                serde_json::json!(expected_levels),
                "{model_id} {interface}"
            );
        }
    }

    // Keep the rejected Spark off level outside both public interfaces.
    let spark = serde_json::to_value(
        registry
            .public_model("gpt-5.3-codex-spark")
            .expect("Spark Public Model must exist")
            .info(),
    )
    .unwrap();
    for interface in ["chat_completions", "responses"] {
        assert_eq!(
            spark["interfaces"][interface]["reasoning"]["levels"],
            serde_json::json!(["low", "medium", "high", "xhigh"]),
            "Spark {interface}"
        );
    }
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
fn hermes_reasoning_include_is_plannable_on_target_responses_interfaces() {
    // Compile the exact GLM Bridge, DeepSeek multi-source Native, and MiMo Native interfaces.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    for (model_id, expected_candidates, expects_bridge) in [
        ("glm-5.2", 1, true),
        ("deepseek-v4-flash", 2, false),
        ("mimo-v2.5", 1, false),
        ("gpt-5.6-luna", 1, false),
    ] {
        // Publish the conditional Responses projection only when every fixed candidate can accept it.
        let model = registry
            .public_model(model_id)
            .unwrap_or_else(|| panic!("{model_id} Public Model must exist"));
        let info = serde_json::to_value(model.info()).unwrap();
        let interface = &info["interfaces"]["responses"];
        assert_eq!(
            interface["response_includes"],
            serde_json::json!(["reasoning.encrypted_content"]),
            "{model_id}"
        );
        assert!(
            interface["supported_parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|parameter| parameter == "include"),
            "{model_id}"
        );

        // Preserve the include on Native requests and consume it only at a Responses-to-Chat Bridge.
        let body = bytes::Bytes::from(
            serde_json::to_vec(&serde_json::json!({
                "model": model_id,
                "input": "hello",
                "include": ["reasoning.encrypted_content"]
            }))
            .unwrap(),
        );
        let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).expect("include must be plannable");
        assert_eq!(plan.candidates().len(), expected_candidates, "{model_id}");
        for candidate in plan.candidates() {
            assert_eq!(candidate.bridge().is_some(), expects_bridge, "{model_id}");
            let upstream: serde_json::Value =
                serde_json::from_slice(candidate.request().body()).unwrap();
            if expects_bridge {
                assert!(upstream.get("include").is_none(), "{model_id}");
            } else {
                assert_eq!(
                    upstream["include"],
                    serde_json::json!(["reasoning.encrypted_content"]),
                    "{model_id}"
                );
            }
        }
    }
}

#[test]
fn hermes_parallel_tool_calls_is_plannable_on_every_verified_candidate() {
    // Compile the exact GLM Bridge, DeepSeek multi-source, and MiMo Native interfaces used by Hermes.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    for (model_id, protocol, interface_name, expected_candidates) in [
        (
            "glm-5.2",
            ApiProtocol::ChatCompletions,
            "chat_completions",
            1,
        ),
        ("glm-5.2", ApiProtocol::Responses, "responses", 1),
        (
            "deepseek-v4-flash",
            ApiProtocol::ChatCompletions,
            "chat_completions",
            3,
        ),
        ("deepseek-v4-flash", ApiProtocol::Responses, "responses", 2),
        (
            "mimo-v2.5",
            ApiProtocol::ChatCompletions,
            "chat_completions",
            1,
        ),
        ("mimo-v2.5", ApiProtocol::Responses, "responses", 1),
    ] {
        // Publish the control only when the complete fixed candidate set accepts it.
        let model = registry
            .public_model(model_id)
            .unwrap_or_else(|| panic!("{model_id} Public Model must exist"));
        let info = serde_json::to_value(model.info()).unwrap();
        let interface = &info["interfaces"][interface_name];
        assert_eq!(
            interface["tools"]["parallel_calls"], "supported",
            "{model_id} {interface_name}"
        );
        assert!(
            interface["supported_parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|parameter| parameter == "parallel_tool_calls"),
            "{model_id} {interface_name}"
        );

        // Preserve true on every Native candidate and through the Responses-to-Chat Bridge.
        let body = match protocol {
            ApiProtocol::ChatCompletions => serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": "Call the synthetic tool."}],
                "tools": [{
                    "type": "function",
                    "function": {"name": "report_result", "parameters": {"type": "object"}}
                }],
                "tool_choice": "auto",
                "parallel_tool_calls": true
            }),
            ApiProtocol::Responses => serde_json::json!({
                "model": model_id,
                "input": "Call the synthetic tool.",
                "tools": [{
                    "type": "function",
                    "name": "report_result",
                    "parameters": {"type": "object"}
                }],
                "tool_choice": "auto",
                "parallel_tool_calls": true
            }),
        };
        let body = bytes::Bytes::from(serde_json::to_vec(&body).unwrap());
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body)
            .unwrap_or_else(|error| panic!("{model_id} {interface_name}: {error:?}"));
        assert_eq!(
            plan.candidates().len(),
            expected_candidates,
            "{model_id} {interface_name}"
        );
        for candidate in plan.candidates() {
            let upstream: serde_json::Value =
                serde_json::from_slice(candidate.request().body()).unwrap();
            assert_eq!(
                upstream["parallel_tool_calls"],
                true,
                "{model_id} {interface_name} {}",
                candidate.route_id()
            );
        }
    }

    // Keep neighboring unverified complete candidate sets fail closed.
    for (model_id, interface_name) in [
        ("deepseek-v4-pro", "chat_completions"),
        ("mimo-v2.5-pro", "chat_completions"),
        ("mimo-v2.5-pro", "responses"),
    ] {
        let info = serde_json::to_value(registry.public_model(model_id).unwrap().info()).unwrap();
        assert_eq!(
            info["interfaces"][interface_name]["tools"]["parallel_calls"], "unsupported",
            "{model_id} {interface_name}"
        );
    }
}

#[test]
fn hermes_stream_usage_is_chat_only_on_every_verified_candidate() {
    // Compile the exact GLM, DeepSeek Flash, and MiMo Chat candidates exercised by Hermes.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    for (model_id, expected_candidates) in
        [("glm-5.2", 1), ("deepseek-v4-flash", 3), ("mimo-v2.5", 1)]
    {
        // Publish stream_options only on the complete fixed Chat interface candidate set.
        let model = registry
            .public_model(model_id)
            .unwrap_or_else(|| panic!("{model_id} Public Model must exist"));
        let info = serde_json::to_value(model.info()).unwrap();
        let chat_parameters = info["interfaces"]["chat_completions"]["supported_parameters"]
            .as_array()
            .unwrap();
        let responses_parameters = info["interfaces"]["responses"]["supported_parameters"]
            .as_array()
            .unwrap();
        assert!(
            chat_parameters
                .iter()
                .any(|parameter| parameter == "stream_options"),
            "{model_id} Chat"
        );
        assert!(
            !responses_parameters
                .iter()
                .any(|parameter| parameter == "stream_options"),
            "{model_id} Responses"
        );

        // Preserve the exact verified option on every Native Chat candidate without reordering.
        let body = bytes::Bytes::from(
            serde_json::to_vec(&serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": "hello"}],
                "stream": true,
                "stream_options": {"include_usage": true}
            }))
            .unwrap(),
        );
        let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
        let plan = plan_request(&registry, &profile, body)
            .unwrap_or_else(|error| panic!("{model_id} Chat: {error:?}"));
        assert_eq!(plan.candidates().len(), expected_candidates, "{model_id}");
        for candidate in plan.candidates() {
            assert!(candidate.bridge().is_none(), "{model_id}");
            let upstream: serde_json::Value =
                serde_json::from_slice(candidate.request().body()).unwrap();
            assert_eq!(
                upstream["stream_options"],
                serde_json::json!({"include_usage": true}),
                "{model_id} {}",
                candidate.route_id()
            );
        }
    }

    // Keep adjacent unverified models and every Responses interface fail closed.
    for model_id in [
        "deepseek-v4-pro",
        "mimo-v2.5-pro",
        "minimax-m3",
        "gpt-5.6-luna",
    ] {
        let info = serde_json::to_value(registry.public_model(model_id).unwrap().info()).unwrap();
        let parameters = info["interfaces"]["chat_completions"]["supported_parameters"]
            .as_array()
            .unwrap();
        assert!(
            !parameters
                .iter()
                .any(|parameter| parameter == "stream_options"),
            "{model_id}"
        );
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
            "supported"
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
                        r#"{{"model":"{model_id}","messages":[{{"role":"user","content":"hello"}}],"reasoning_effort":"{level}"}}"#
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
fn every_general_generation_candidate_uses_the_gateway_instruction_and_store_envelope() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let default = "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed.";
    let mut checked_candidates = 0;

    for public_model in registry.public_models() {
        let model_id = public_model.standard().id();
        let info = serde_json::to_value(public_model.info()).unwrap();
        if !info["capabilities"]["tasks"]
            .as_array()
            .is_some_and(|tasks| tasks.iter().any(|task| task == "text_generation"))
        {
            continue;
        }

        for (protocol, body, expected) in [
            (
                ApiProtocol::ChatCompletions,
                serde_json::json!({
                    "model": model_id,
                    "messages": [{"role": "user", "content": "hello"}]
                }),
                default,
            ),
            (
                ApiProtocol::Responses,
                serde_json::json!({
                    "model": model_id,
                    "input": "hello",
                    "instructions": "  client instruction  "
                }),
                "  client instruction  ",
            ),
        ] {
            let interface_name = match protocol {
                ApiProtocol::ChatCompletions => "chat_completions",
                ApiProtocol::Responses => "responses",
            };
            if info["interfaces"][interface_name].is_null() {
                continue;
            }
            let body = bytes::Bytes::from(serde_json::to_vec(&body).unwrap());
            let profile = analyze_request(protocol, &body).unwrap();
            let plan = plan_request(&registry, &profile, body)
                .unwrap_or_else(|error| panic!("{model_id} {interface_name}: {error:?}"));

            for candidate in plan.candidates() {
                checked_candidates += 1;
                let request: serde_json::Value =
                    serde_json::from_slice(candidate.request().body()).unwrap();
                match candidate.request().protocol() {
                    ApiProtocol::Responses => {
                        assert_eq!(request["instructions"], expected, "{model_id}");
                        assert_eq!(request["store"], false, "{model_id}");
                    }
                    ApiProtocol::ChatCompletions => {
                        assert_eq!(request["messages"][0]["content"], expected, "{model_id}");
                    }
                }
            }
        }
    }
    assert!(checked_candidates > 0);
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
