//! Verifies that example configuration, the compiled model catalog, and default route facts remain consistent.

use openbridge::{
    config::parse_bootstrap_config,
    core::{ApiProtocol, ReasoningOutput},
    identity::UserConfigPath,
    pipeline::{analyze_request, plan_request},
    provider::ProviderKind,
    providers::{build_compiled_registry, compiled_config},
    registry::{
        InputModality, ModelMode, OutputModality, ReasoningLevel, ReasoningSupport, RouteConfig,
        RouteMode, UpstreamApiCapabilities, build_registry,
    },
    upstream_credentials::UpstreamCredentialConfiguration,
};

#[test]
fn compiled_model_catalog_includes_litellm_text_models() {
    let definition = compiled_config();
    let expected = [
        "meituan/longcat-2.0",
        "openai/gpt-5.6-sol",
        "openai/gpt-5.6-terra",
        "openai/gpt-5.6-luna",
        "openai/gpt-5.5",
        "openai/gpt-5.3-codex-spark",
        "openai/text-embedding-3-small",
        "deepseek/deepseek-v4-pro",
        "deepseek/deepseek-v4-flash",
        "xiaomi/mimo-v2.5-pro",
        "xiaomi/mimo-v2.5",
        "qwen/qwen3.7-max",
        "qwen/qwen3.7-plus",
        "z-ai/glm-5.2",
        "moonshotai/kimi-k3",
        "minimax/minimax-m3",
    ];

    // Moving family and version modules must not change catalog contents or stable order.
    let actual = definition
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(
        definition
            .models
            .iter()
            .all(|model| model.id != "openai/configured-model")
    );

    // Every model has an official catalog description except Codex Spark, which OpenRouter does not list precisely.
    assert!(
        definition.models.iter().all(|model| {
            model.id == "openai/gpt-5.3-codex-spark" || model.description.is_some()
        })
    );

    let longcat = definition
        .models
        .iter()
        .find(|model| model.id == "meituan/longcat-2.0")
        .expect("OpenRouter LongCat id is canonical");
    assert_eq!(longcat.context_length.context_tokens(), Some(1_048_756));
    assert_eq!(longcat.context_length.input_tokens(), Some(1_048_756));
    assert_eq!(longcat.context_length.output_tokens(), Some(262_144));
    assert_eq!(longcat.mode, Some(ModelMode::Chat));
    assert_eq!(longcat.input_modalities, Some(vec![InputModality::Text]));
    assert_eq!(longcat.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(longcat.tokenizer.as_deref(), Some("Other"));
    assert_eq!(longcat.knowledge_cutoff, None);

    let sol = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.6-sol")
        .unwrap();
    assert_eq!(sol.context_length.context_tokens(), Some(1_050_000));
    assert_eq!(sol.context_length.input_tokens(), Some(1_050_000));
    assert_eq!(sol.context_length.output_tokens(), Some(128_000));
    assert_eq!(sol.mode, Some(ModelMode::Chat));
    assert_eq!(
        sol.input_modalities,
        Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::File
        ])
    );
    assert_eq!(sol.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(sol.tokenizer.as_deref(), Some("GPT"));
    assert_eq!(sol.knowledge_cutoff.as_deref(), Some("2026-02-16"));
    assert_eq!(
        sol.reasoning_levels,
        [
            ReasoningLevel::Max,
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ]
    );

    let gpt_5_5 = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.5")
        .unwrap();
    assert_eq!(
        gpt_5_5.reasoning_levels,
        [
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ]
    );

    let codex_spark = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.3-codex-spark")
        .unwrap();
    assert_eq!(codex_spark.context_length.context_tokens(), Some(128_000));
    assert_eq!(codex_spark.context_length.input_tokens(), None);
    assert_eq!(codex_spark.context_length.output_tokens(), Some(128_000));
    assert_eq!(
        codex_spark.reasoning_levels,
        [
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
        ]
    );

    // Representative models retain context, output limits, and standard reasoning levels.
    let deepseek = definition
        .models
        .iter()
        .find(|model| model.id == "deepseek/deepseek-v4-pro")
        .unwrap();
    assert_eq!(deepseek.context_length.context_tokens(), Some(1_048_576));
    assert_eq!(deepseek.context_length.input_tokens(), Some(1_048_576));
    assert_eq!(deepseek.context_length.output_tokens(), Some(384_000));
    assert_eq!(deepseek.mode, Some(ModelMode::Chat));
    assert_eq!(deepseek.input_modalities, Some(vec![InputModality::Text]));
    assert_eq!(deepseek.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(deepseek.tokenizer.as_deref(), Some("DeepSeek"));
    assert_eq!(
        deepseek.reasoning_levels,
        [ReasoningLevel::XHigh, ReasoningLevel::High]
    );

    let deepseek_flash = definition
        .models
        .iter()
        .find(|model| model.id == "deepseek/deepseek-v4-flash")
        .unwrap();
    assert_eq!(deepseek_flash.context_length.output_tokens(), Some(393_216));

    // Keep unrelated rerank models out until that task and protocol have an executable contract.
    assert!(
        definition
            .models
            .iter()
            .all(|model| !model.id.contains("rerank"))
    );
}

#[test]
fn requested_public_model_and_provider_matrix_is_compiled() {
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let definition = compiled_config();

    // Keep removed model facts out of the compiled canonical catalog.
    assert!(
        !definition
            .models
            .iter()
            .any(|model| model.id == "tencent/hy3")
    );
    assert!(
        !definition
            .models
            .iter()
            .any(|model| model.id == "nvidia/nemotron-3-ultra-550b-a55b")
    );

    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Publish the actual OpenAI model names instead of deployment aliases.
    assert!(registry.public_model("gpt-5.6-sol").is_some());
    assert!(registry.public_model("text-embedding-3-small").is_some());
    assert!(registry.public_model("code-primary").is_none());
    assert!(registry.public_model("embedding-primary").is_none());

    // Replace the OpenRouter Nemotron target with a DeepSeek V4 Flash target.
    assert!(
        registry
            .upstream_target("openrouter-nemotron-3-ultra")
            .is_none()
    );
    let openrouter_flash = registry
        .upstream_target("openrouter-deepseek-v4-flash")
        .expect("OpenRouter DeepSeek V4 Flash target should be compiled");
    assert_eq!(openrouter_flash.kind(), ProviderKind::OpenRouter);
    assert_eq!(openrouter_flash.model_id(), "deepseek/deepseek-v4-flash");
    assert_eq!(
        openrouter_flash
            .upstream_api("responses")
            .unwrap()
            .upstream_model(),
        "deepseek/deepseek-v4-flash"
    );

    // Keep Pro Chat-only while Flash obtains a Native Responses candidate from OpenRouter.
    let pro = registry
        .public_model("deepseek-v4-pro")
        .expect("DeepSeek V4 Pro should remain visible");
    assert_eq!(pro.routes(), ["deepseek-v4-pro-deepseek-chat"]);
    let pro_info = serde_json::to_value(pro.info()).unwrap();
    assert_eq!(pro_info["interfaces"]["responses"], serde_json::Value::Null);

    let flash = registry
        .public_model("deepseek-v4-flash")
        .expect("DeepSeek V4 Flash should remain visible");
    assert_eq!(
        flash.routes(),
        [
            "deepseek-v4-flash-deepseek-chat",
            "deepseek-v4-flash-openrouter-chat",
            "deepseek-v4-flash-openrouter-responses",
        ]
    );
    let responses = bytes::Bytes::from_static(
        br#"{"model":"deepseek-v4-flash","input":"hello","stream":true}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &responses).unwrap();
    let plan = plan_request(&registry, &profile, responses).unwrap();
    assert_eq!(
        plan.candidates()[0].route_id(),
        "deepseek-v4-flash-openrouter-responses"
    );
    assert_eq!(
        plan.candidates()[0].upstream_target_id(),
        "openrouter-deepseek-v4-flash"
    );
}

#[test]
fn checked_in_bootstrap_and_compiled_registry_are_loadable() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap =
        parse_bootstrap_config(bootstrap).expect("checked-in bootstrap must remain valid");
    let bootstrap_template = include_str!("../config/bootstrap.example.toml");
    let bootstrap_template = parse_bootstrap_config(bootstrap_template)
        .expect("checked-in bootstrap template must remain valid");
    assert_eq!(bootstrap_template, bootstrap);
    let registry =
        build_compiled_registry(bootstrap).expect("compiled registry must remain internally valid");

    assert_eq!(registry.version().as_str(), "dev-1");
    assert!(registry.listen().ip().is_loopback());
    let users = UserConfigPath::new("config/users.example.toml")
        .load()
        .expect("checked-in user example must remain valid");
    assert_eq!(users.users().users().next().unwrap().id(), "local-user");
    assert_eq!(
        registry
            .public_model("gpt-5.6-sol")
            .expect("public model is compiled")
            .routes(),
        [
            "gpt-5.6-sol-openai-chat",
            "gpt-5.6-sol-openai-chat-via-responses",
            "gpt-5.6-sol-openai-responses",
            "gpt-5.6-sol-openai-responses-via-chat",
        ]
    );

    let longcat = registry
        .public_model("LongCat-2.0")
        .expect("LongCat public model is compiled");
    assert_eq!(longcat.routes().len(), 4);
    let target = registry
        .upstream_target("longcat-2")
        .expect("LongCat target is compiled");
    let chat = target.upstream_api("chat").unwrap();
    assert_eq!(target.kind(), ProviderKind::LongCat);
    assert_eq!(chat.upstream_model(), "LongCat-2.0");
    assert_eq!(chat.reasoning_output(), ReasoningOutput::Unknown);
    assert_eq!(
        target.upstream_api("responses").unwrap().reasoning_output(),
        ReasoningOutput::Unknown
    );
    assert_eq!(target.endpoint_base().as_str(), "https://api.longcat.chat/");
    assert_eq!(
        chat.model().context_length().context_tokens(),
        Some(1_048_756)
    );
    assert_eq!(
        chat.model().context_length().input_tokens(),
        Some(1_048_756)
    );
    assert_eq!(chat.model().context_length().output_tokens(), Some(262_144));
    assert_eq!(chat.model().reasoning(), ReasoningSupport::Supported);
    assert!(
        chat.model()
            .supported_parameters()
            .iter()
            .any(|parameter| parameter == "tools")
    );
    assert!(
        chat.model()
            .supported_parameters()
            .iter()
            .any(|parameter| parameter == "reasoning")
    );

    let openai = registry
        .upstream_target("openai-main")
        .expect("OpenAI target is compiled");
    assert_eq!(
        openai.upstream_api("chat").unwrap().model().id(),
        "openai/gpt-5.6-sol"
    );
    assert_eq!(
        openai.upstream_api("chat").unwrap().upstream_model(),
        "gpt-5.6-sol"
    );

    let openrouter_public = registry
        .public_model("deepseek-v4-flash")
        .expect("OpenRouter DeepSeek V4 Flash public model is compiled");
    assert_eq!(
        openrouter_public.routes(),
        [
            "deepseek-v4-flash-deepseek-chat",
            "deepseek-v4-flash-openrouter-chat",
            "deepseek-v4-flash-openrouter-responses"
        ]
    );
    let openrouter = registry
        .upstream_target("openrouter-deepseek-v4-flash")
        .expect("OpenRouter DeepSeek V4 Flash target is compiled");
    assert_eq!(openrouter.kind(), ProviderKind::OpenRouter);
    assert_eq!(openrouter.model_id(), "deepseek/deepseek-v4-flash");
    assert_eq!(openrouter.credential_pool_id(), "openrouter-primary");
    assert!(registry.credential_pool("openrouter-primary").is_some());
    assert_eq!(
        openrouter.endpoint_base().as_str(),
        "https://openrouter.ai/api/v1/"
    );
    let openrouter_chat = openrouter.upstream_api("chat").unwrap();
    assert_eq!(
        openrouter_chat.upstream_model(),
        "deepseek/deepseek-v4-flash"
    );
    let openrouter_responses = openrouter.upstream_api("responses").unwrap();
    assert_eq!(
        openrouter_responses.upstream_model(),
        "deepseek/deepseek-v4-flash"
    );
    let responses_capabilities = match openrouter_responses.capabilities() {
        UpstreamApiCapabilities::Responses(capabilities) => capabilities,
        UpstreamApiCapabilities::ChatCompletions(_) => panic!("expected Responses capabilities"),
        UpstreamApiCapabilities::Embeddings(_) => panic!("expected Responses capabilities"),
    };
    assert!(responses_capabilities.enabled);
    assert!(responses_capabilities.streaming);
    assert!(responses_capabilities.function_calling);
    assert!(!responses_capabilities.store);
    assert!(!responses_capabilities.previous_response_id);
    assert!(!responses_capabilities.background);

    let body = bytes::Bytes::from_static(
        br#"{"model":"deepseek-v4-flash","messages":[],"reasoning_effort":"high","tools":[{"type":"function","function":{"name":"probe"}}]}"#,
    );
    let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(
        plan.candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        [
            "deepseek-v4-flash-deepseek-chat",
            "deepseek-v4-flash-openrouter-chat"
        ]
    );

    let responses = bytes::Bytes::from_static(
        br#"{"model":"deepseek-v4-flash","input":"hello","stream":true,"reasoning":{"effort":"high"},"tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}]}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &responses).unwrap();
    let plan = plan_request(&registry, &profile, responses).unwrap();
    assert_eq!(
        plan.candidates()[0].route_id(),
        "deepseek-v4-flash-openrouter-responses"
    );

    for unsupported in [
        br#"{"model":"deepseek-v4-flash","input":"hello","store":true}"#.as_slice(),
        br#"{"model":"deepseek-v4-flash","input":"hello","previous_response_id":"resp_123"}"#
            .as_slice(),
        br#"{"model":"deepseek-v4-flash","input":"hello","background":true}"#.as_slice(),
    ] {
        let body = bytes::Bytes::copy_from_slice(unsupported);
        let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
        assert!(matches!(
            plan_request(&registry, &profile, body),
            Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
        ));
    }

    for (protocol, body) in [
        (
            ApiProtocol::ChatCompletions,
            r#"{"model":"LongCat-2.0","messages":[]}"#,
        ),
        (
            ApiProtocol::Responses,
            r#"{"model":"LongCat-2.0","input":"hello"}"#,
        ),
        (
            ApiProtocol::ChatCompletions,
            r#"{"model":"LongCat-2.0","messages":[],"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
        ),
        (
            ApiProtocol::Responses,
            r#"{"model":"LongCat-2.0","input":"hello","tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}]}"#,
        ),
    ] {
        let body = bytes::Bytes::copy_from_slice(body.as_bytes());
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body)
            .expect("LongCat should remain on the native path for both protocols");
        assert_eq!(plan.upstream_target_id(), "longcat-2");
    }

    // Reverse-Bridge reasoning output is unverified, so the fixed Responses contract cannot allow Native alone.
    let body = bytes::Bytes::from(r#"{"model":"LongCat-2.0","input":"hello","reasoning":{}}"#);
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    assert!(matches!(
        plan_request(&registry, &profile, body),
        Err(openbridge::pipeline::RequestPlanningError::ReasoningUnsupported)
    ));
}

#[test]
fn deepseek_models_keep_chat_only_while_flash_uses_openrouter_responses() {
    // Build the complete compiled registry and check the fixed trusted boundaries of both DeepSeek targets.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    for (public_name, target_id, canonical_model) in [
        (
            "deepseek-v4-pro",
            "deepseek-v4-pro",
            "deepseek/deepseek-v4-pro",
        ),
        (
            "deepseek-v4-flash",
            "deepseek-v4-flash",
            "deepseek/deepseek-v4-flash",
        ),
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("DeepSeek target should be compiled");
        assert_eq!(target.kind(), ProviderKind::DeepSeek);
        assert_eq!(target.model_id(), canonical_model);
        assert_eq!(target.endpoint_base().as_str(), "https://api.deepseek.com/");
        assert_eq!(target.quota_scope(), Some("deepseek-primary"));
        assert_eq!(target.fault_domain(), Some("deepseek-api"));
        assert_eq!(target.credential_pool_id(), "deepseek-primary");
        assert!(
            registry
                .credential_pool(target.credential_pool_id())
                .is_some()
        );
        assert_eq!(
            target.upstream_api("chat").unwrap().upstream_model(),
            public_name
        );
        assert_eq!(
            target.upstream_api("chat").unwrap().reasoning_output(),
            ReasoningOutput::PlainText
        );
        assert!(target.upstream_api("responses").is_none());

        // Verify that downstream Chat retains the direct DeepSeek Native candidate.
        let public_model = registry
            .public_model(public_name)
            .expect("DeepSeek Public Model should be compiled");
        let chat = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","messages":[{{"role":"user","content":"hello"}}]}}"#
        ));
        let profile = analyze_request(ApiProtocol::ChatCompletions, &chat).unwrap();
        let plan = plan_request(&registry, &profile, chat).unwrap();
        assert_eq!(
            plan.candidates()[0].route_id(),
            format!("{public_name}-deepseek-chat")
        );

        let info = serde_json::to_value(public_model.info()).unwrap();
        if public_name == "deepseek-v4-pro" {
            assert_eq!(public_model.routes(), ["deepseek-v4-pro-deepseek-chat"]);
            assert_eq!(info["interfaces"]["responses"], serde_json::Value::Null);
        } else {
            // Flash aggregates direct DeepSeek Chat with OpenRouter Chat/Responses Native routes.
            assert_eq!(
                public_model.routes(),
                [
                    "deepseek-v4-flash-deepseek-chat",
                    "deepseek-v4-flash-openrouter-chat",
                    "deepseek-v4-flash-openrouter-responses",
                ]
            );
            let responses =
                bytes::Bytes::from_static(br#"{"model":"deepseek-v4-flash","input":"hello"}"#);
            let profile = analyze_request(ApiProtocol::Responses, &responses).unwrap();
            let plan = plan_request(&registry, &profile, responses).unwrap();
            assert_eq!(
                plan.candidates()[0].route_id(),
                "deepseek-v4-flash-openrouter-responses"
            );
            assert!(info["interfaces"]["responses"].is_object());
        }
    }
}

#[test]
fn compiled_reasoning_output_types_match_deepseek_flash_and_mimo_v25_routes() {
    // Build the complete compiled registry and read the actual targets and Provider API classes for two models.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    let deepseek = registry
        .upstream_target("deepseek-v4-flash")
        .expect("DeepSeek V4 Flash target should be compiled");
    assert_eq!(
        deepseek.upstream_api("chat").unwrap().reasoning_output(),
        ReasoningOutput::PlainText
    );
    assert!(deepseek.upstream_api("responses").is_none());

    // DeepSeek's direct target remains Chat-only; Flash Responses is served by OpenRouter Native.
    let deepseek_body =
        bytes::Bytes::from(r#"{"model":"deepseek-v4-flash","input":"hello","reasoning":{}}"#);
    let deepseek_profile = analyze_request(ApiProtocol::Responses, &deepseek_body).unwrap();
    let deepseek_plan = plan_request(&registry, &deepseek_profile, deepseek_body).unwrap();
    assert_eq!(
        deepseek_plan.candidates()[0].route_id(),
        "deepseek-v4-flash-openrouter-responses"
    );
    assert_eq!(deepseek_plan.candidates()[0].upstream_api_id(), "responses");
    assert!(deepseek_plan.candidates()[0].bridge().is_none());

    let mimo = registry
        .upstream_target("mimo-v2-5")
        .expect("MiMo V2.5 target should be compiled");
    assert_eq!(
        mimo.upstream_api("chat").unwrap().reasoning_output(),
        ReasoningOutput::Unknown
    );
    assert_eq!(
        mimo.upstream_api("responses").unwrap().reasoning_output(),
        ReasoningOutput::Unknown
    );

    // MiMo Bridge cannot safely represent existing reasoning items, so the fixed contract cannot skip its Bridge candidate.
    let mimo_body = bytes::Bytes::from(
        r#"{"model":"mimo-v2.5","input":[{"type":"reasoning","id":"rs_1","summary":[{"type":"summary_text","text":"prior"}]}]}"#,
    );
    let mimo_profile = analyze_request(ApiProtocol::Responses, &mimo_body).unwrap();
    assert!(matches!(
        plan_request(&registry, &mimo_profile, mimo_body),
        Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
    ));
}

#[test]
fn mimo_models_are_compiled_with_dual_native_first_routes() {
    // Build the complete compiled registry and check the fixed trusted boundaries of both MiMo targets.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    for (public_name, target_id, canonical_model, route_prefix) in [
        (
            "mimo-v2.5-pro",
            "mimo-v2-5-pro",
            "xiaomi/mimo-v2.5-pro",
            "mimo-v2-5-pro-mimo",
        ),
        (
            "mimo-v2.5",
            "mimo-v2-5",
            "xiaomi/mimo-v2.5",
            "mimo-v2-5-mimo",
        ),
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("MiMo target should be compiled");
        assert_eq!(target.kind(), ProviderKind::MiMo);
        assert_eq!(target.model_id(), canonical_model);
        assert_eq!(
            target.endpoint_base().as_str(),
            "https://api.xiaomimimo.com/"
        );
        assert_eq!(target.quota_scope(), Some("mimo-primary"));
        assert_eq!(target.fault_domain(), Some("mimo-api"));
        assert_eq!(target.credential_pool_id(), "mimo-primary");
        assert!(
            registry
                .credential_pool(target.credential_pool_id())
                .is_some()
        );
        assert_eq!(
            target.upstream_api("chat").unwrap().upstream_model(),
            public_name
        );
        assert_eq!(
            target.upstream_api("responses").unwrap().upstream_model(),
            public_name
        );
        assert_eq!(
            target.upstream_api("chat").unwrap().reasoning_output(),
            ReasoningOutput::Unknown
        );
        assert_eq!(
            target.upstream_api("responses").unwrap().reasoning_output(),
            ReasoningOutput::Unknown
        );

        // Verify that both compiled Native APIs share the MiMo Provider contract's capability boundary.
        let chat_capabilities = match target.upstream_api("chat").unwrap().capabilities() {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities,
            UpstreamApiCapabilities::Responses(_) => panic!("expected Chat capabilities"),
            UpstreamApiCapabilities::Embeddings(_) => panic!("expected Chat capabilities"),
        };
        assert!(chat_capabilities.parallel_tool_calls);
        assert!(chat_capabilities.image_input);
        assert!(chat_capabilities.structured_outputs);
        assert!(!chat_capabilities.store);
        let responses_capabilities = match target.upstream_api("responses").unwrap().capabilities()
        {
            UpstreamApiCapabilities::Responses(capabilities) => capabilities,
            UpstreamApiCapabilities::ChatCompletions(_) => {
                panic!("expected Responses capabilities")
            }
            UpstreamApiCapabilities::Embeddings(_) => {
                panic!("expected Responses capabilities")
            }
        };
        assert!(responses_capabilities.parallel_tool_calls);
        assert!(responses_capabilities.image_input);
        assert!(responses_capabilities.structured_outputs);
        assert!(!responses_capabilities.store);
        assert!(!responses_capabilities.previous_response_id);
        assert!(!responses_capabilities.background);

        // Verify Native-first, reverse-Bridge-second ordering for both downstream protocols.
        let public_model = registry
            .public_model(public_name)
            .expect("MiMo Public Model should be compiled");
        assert_eq!(
            public_model.routes(),
            [
                format!("{route_prefix}-chat"),
                format!("{route_prefix}-chat-via-responses"),
                format!("{route_prefix}-responses"),
                format!("{route_prefix}-responses-via-chat"),
            ]
        );
        for (protocol, body, expected_route) in [
            (
                ApiProtocol::ChatCompletions,
                format!(r#"{{"model":"{public_name}","messages":[]}}"#),
                format!("{route_prefix}-chat"),
            ),
            (
                ApiProtocol::Responses,
                format!(r#"{{"model":"{public_name}","input":"hello"}}"#),
                format!("{route_prefix}-responses"),
            ),
        ] {
            let body = bytes::Bytes::from(body);
            let profile = analyze_request(protocol, &body).unwrap();
            let plan = plan_request(&registry, &profile, body).unwrap();
            assert_eq!(plan.candidates()[0].route_id(), expected_route);
        }

        // Function tools are shared by both complete Routes, preserving Native-first and Bridge fallback ordering.
        for (protocol, body, expected_route) in [
            (
                ApiProtocol::ChatCompletions,
                format!(
                    r#"{{"model":"{public_name}","messages":[],"tools":[{{"type":"function","function":{{"name":"lookup","parameters":{{"type":"object"}}}}}}],"parallel_tool_calls":true}}"#
                ),
                format!("{route_prefix}-chat"),
            ),
            (
                ApiProtocol::Responses,
                format!(
                    r#"{{"model":"{public_name}","input":"lookup","tools":[{{"type":"function","name":"lookup","parameters":{{"type":"object"}}}}],"parallel_tool_calls":true}}"#
                ),
                format!("{route_prefix}-responses"),
            ),
        ] {
            let body = bytes::Bytes::from(body);
            let profile = analyze_request(protocol, &body).unwrap();
            let plan = plan_request(&registry, &profile, body).unwrap();
            assert_eq!(plan.candidates()[0].route_id(), expected_route);
            assert_eq!(plan.candidates().len(), 2);
        }

        // Image and structured output are not fully shared by the reverse Bridge, so the fixed contract rejects both.
        for (protocol, body) in [
            (
                ApiProtocol::ChatCompletions,
                format!(
                    r#"{{"model":"{public_name}","messages":[{{"role":"user","content":[{{"type":"image_url","image_url":{{"url":"https://example.invalid/image.png"}}}}]}}]}}"#
                ),
            ),
            (
                ApiProtocol::ChatCompletions,
                format!(
                    r#"{{"model":"{public_name}","messages":[],"response_format":{{"type":"json_schema","json_schema":{{"name":"answer","schema":{{"type":"object"}}}}}}}}"#
                ),
            ),
            (
                ApiProtocol::Responses,
                format!(
                    r#"{{"model":"{public_name}","input":[{{"type":"input_image","image_url":"https://example.invalid/image.png"}}]}}"#
                ),
            ),
            (
                ApiProtocol::Responses,
                format!(
                    r#"{{"model":"{public_name}","input":"return json","text":{{"format":{{"type":"json_schema","name":"answer","schema":{{"type":"object"}}}}}}}}"#
                ),
            ),
        ] {
            let body = bytes::Bytes::from(body);
            let profile = analyze_request(protocol, &body).unwrap();
            assert!(matches!(
                plan_request(&registry, &profile, body),
                Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
            ));
        }

        // Verify that MiMo's stateless boundary still rejects stateful Responses requests.
        for body in [
            format!(r#"{{"model":"{public_name}","input":"hello","store":true}}"#),
            format!(
                r#"{{"model":"{public_name}","input":"hello","previous_response_id":"resp_123"}}"#
            ),
            format!(r#"{{"model":"{public_name}","input":"hello","background":true}}"#),
        ] {
            let body = bytes::Bytes::from(body);
            let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
            assert!(matches!(
                plan_request(&registry, &profile, body),
                Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
            ));
        }
    }
}

#[test]
fn compiled_provider_credential_pools_are_shared_and_match_the_private_toml_example() {
    // Build the complete registry and load every credential pool from a TOML template with no real values.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let pool_ids = registry
        .credential_pool_ids()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let credentials = UpstreamCredentialConfiguration::from_toml(include_str!(
        "../config/upstream-credentials.example.toml"
    ))
    .unwrap()
    .into_builder_for(&registry, pool_ids.iter().map(String::as_str))
    .unwrap()
    .build();

    // Verify that each target retrieves the template credential by Provider and pool.
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        assert!(
            credentials
                .upstream_pool(
                    target.kind(),
                    target.credential_pool_id(),
                    registry
                        .credential_pool(target.credential_pool_id())
                        .unwrap()
                        .kind(),
                )
                .is_ok()
        );
    }
}

#[test]
fn compiled_registry_can_select_each_protocol_bridge_when_the_native_api_is_unavailable() {
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let mut definition = compiled_config();
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "openai-main")
        .unwrap();

    // Disable Chat Native capability so downstream Chat requests must use the Responses bridge.
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut target.upstream_apis[0].capabilities
    {
        capabilities.enabled = false;
    }
    let registry = build_registry(bootstrap.clone(), definition.clone()).unwrap();
    let body = bytes::Bytes::from_static(
        br#"{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"hello"}]}"#,
    );
    let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(plan.candidates().len(), 1);
    assert_eq!(
        plan.candidates()[0].route_id(),
        "gpt-5.6-sol-openai-chat-via-responses"
    );
    assert!(plan.candidates()[0].bridge().is_some());

    // Disable Responses Native capability so downstream Responses requests must use the Chat bridge.
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "openai-main")
        .unwrap();
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut target.upstream_apis[0].capabilities
    {
        capabilities.enabled = true;
    }
    if let UpstreamApiCapabilities::Responses(capabilities) =
        &mut target.upstream_apis[1].capabilities
    {
        capabilities.enabled = false;
    }
    let registry = build_registry(bootstrap, definition).unwrap();
    let body = bytes::Bytes::from_static(br#"{"model":"gpt-5.6-sol","input":"hello"}"#);
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(plan.candidates().len(), 1);
    assert_eq!(
        plan.candidates()[0].route_id(),
        "gpt-5.6-sol-openai-responses-via-chat"
    );
    assert!(plan.candidates()[0].bridge().is_some());
}

#[test]
fn same_model_routes_are_aggregated_across_providers_in_native_first_order() {
    // Clone the LongCat deployment into an OpenAI-owned target that references the same canonical Model.
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap =
        parse_bootstrap_config(bootstrap).expect("checked-in bootstrap must remain valid");
    let mut definition = compiled_config();
    let mut alternate = definition
        .upstream_targets
        .iter()
        .find(|target| target.id == "longcat-2")
        .expect("LongCat target is compiled")
        .clone();
    alternate.id = "openai-longcat-test".to_owned();
    alternate.provider = ProviderKind::OpenAi;
    alternate.credential_pool = "openai-primary".to_owned();
    for upstream_api in &mut alternate.upstream_apis {
        upstream_api.upstream_model = "longcat/longcat-2.0".to_owned();
        upstream_api.endpoint_profile = "public-api".to_owned();
        match &mut upstream_api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.function_calling = false;
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.function_calling = false;
            }
            UpstreamApiCapabilities::Embeddings(_) => {
                panic!("generation target must not contain Embeddings capabilities")
            }
        }
    }
    alternate.base_url = "https://api.openai.com".to_owned();
    definition.upstream_targets.push(alternate);

    // Add the alternate Provider's complete surface and aggregate both targets Native-first per protocol.
    definition.routes.extend([
        RouteConfig {
            id: "longcat-openai-chat".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_api: "chat".to_owned(),
            downstream_operation: ApiProtocol::ChatCompletions.operation(),
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "longcat-openai-chat-via-responses".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_api: "responses".to_owned(),
            downstream_operation: ApiProtocol::ChatCompletions.operation(),
            mode: RouteMode::Bridged,
        },
        RouteConfig {
            id: "longcat-openai-responses".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_api: "responses".to_owned(),
            downstream_operation: ApiProtocol::Responses.operation(),
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "longcat-openai-responses-via-chat".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_api: "chat".to_owned(),
            downstream_operation: ApiProtocol::Responses.operation(),
            mode: RouteMode::Bridged,
        },
    ]);
    definition
        .public_models
        .iter_mut()
        .find(|model| model.id == "LongCat-2.0")
        .expect("LongCat Public Model is compiled")
        .routes = vec![
        "longcat-2-chat".to_owned(),
        "longcat-openai-chat".to_owned(),
        "longcat-2-chat-via-responses".to_owned(),
        "longcat-openai-chat-via-responses".to_owned(),
        "longcat-2-responses".to_owned(),
        "longcat-openai-responses".to_owned(),
        "longcat-2-responses-via-chat".to_owned(),
        "longcat-openai-responses-via-chat".to_owned(),
    ];

    // Compile the full registry and confirm both Provider targets retain one canonical Model identity.
    let registry = build_registry(bootstrap, definition)
        .expect("different providers may reference one canonical model");
    let direct = registry
        .upstream_target("longcat-2")
        .expect("direct LongCat target exists")
        .upstream_api("chat")
        .unwrap();
    let alternate = registry
        .upstream_target("openai-longcat-test")
        .expect("alternate provider target exists")
        .upstream_api("chat")
        .unwrap();

    assert_eq!(direct.model().id(), "meituan/longcat-2.0");
    assert_eq!(alternate.model().id(), "meituan/longcat-2.0");
    assert_eq!(direct.model(), alternate.model());

    // Plan each protocol in the aggregated fixed order without capability-based candidate selection.
    let chat_body = bytes::Bytes::from_static(
        br#"{"model":"LongCat-2.0","messages":[{"role":"user","content":"hello"}]}"#,
    );
    let chat_profile = analyze_request(ApiProtocol::ChatCompletions, &chat_body).unwrap();
    let chat_plan = plan_request(&registry, &chat_profile, chat_body).unwrap();
    assert_eq!(
        chat_plan
            .candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        [
            "longcat-2-chat",
            "longcat-openai-chat",
            "longcat-2-chat-via-responses",
            "longcat-openai-chat-via-responses",
        ]
    );
    let responses_body = bytes::Bytes::from_static(br#"{"model":"LongCat-2.0","input":"hello"}"#);
    let responses_profile = analyze_request(ApiProtocol::Responses, &responses_body).unwrap();
    let responses_plan = plan_request(&registry, &responses_profile, responses_body).unwrap();
    assert_eq!(
        responses_plan
            .candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        [
            "longcat-2-responses",
            "longcat-openai-responses",
            "longcat-2-responses-via-chat",
            "longcat-openai-responses-via-chat",
        ]
    );

    // Intersect the weaker fallback capability into the public contract and reject tools before egress.
    let info = serde_json::to_value(
        registry
            .public_model("LongCat-2.0")
            .expect("aggregated Public Model exists")
            .info(),
    )
    .unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["tools"]["support"],
        "unsupported"
    );
    let tools_body = bytes::Bytes::from_static(
        br#"{"model":"LongCat-2.0","messages":[],"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
    );
    let tools_profile = analyze_request(ApiProtocol::ChatCompletions, &tools_body).unwrap();
    assert!(matches!(
        plan_request(&registry, &tools_profile, tools_body),
        Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
    ));
}
