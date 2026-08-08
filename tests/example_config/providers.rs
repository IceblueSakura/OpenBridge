//! Verifies compiled Provider targets and their model-specific protocol surfaces.

use super::*;

#[test]
fn nvidia_and_bailian_compile_as_fixed_api_key_provider_profiles() {
    // Locate each fixed Provider instance and its separately owned API-key pool.
    let definition = compiled_config();
    for (provider_id, provider, base_url, pool_id) in [
        (
            "nvidia",
            ProviderKind::Nvidia,
            "https://integrate.api.nvidia.com/v1",
            "nvidia-primary",
        ),
        (
            "bailian",
            ProviderKind::Bailian,
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "bailian-primary",
        ),
    ] {
        let instance = definition
            .provider_instances
            .iter()
            .find(|instance| instance.id == provider_id)
            .expect("fixed Provider instance should be compiled");
        assert_eq!(instance.kind, provider);
        assert_eq!(instance.base_url, base_url);

        let pool = definition
            .credential_pools
            .iter()
            .find(|pool| pool.id == pool_id)
            .expect("Provider API-key pool should be compiled");
        assert_eq!(pool.provider, provider);
        assert_eq!(pool.kind, CredentialKind::ApiKey);
    }

    // Compile the complete registry and retain both fixed Provider credential boundaries.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    assert_eq!(
        registry.provider_instance("nvidia").unwrap().kind(),
        ProviderKind::Nvidia
    );
    assert_eq!(
        registry.provider_instance("bailian").unwrap().kind(),
        ProviderKind::Bailian
    );
    assert!(registry.credential_pool("nvidia-primary").is_some());
    assert!(registry.credential_pool("bailian-primary").is_some());

    // Keep both placeholder bindings visible in the checked-in credential template.
    let credentials = UpstreamCredentialConfiguration::from_toml(include_str!(
        "../../config/upstream-credentials.example.toml"
    ))
    .unwrap();
    let active_pool_ids = credentials.active_pool_ids().collect::<Vec<_>>();
    assert!(active_pool_ids.contains(&"nvidia-primary"));
    assert!(active_pool_ids.contains(&"bailian-primary"));
}

#[test]
fn kimi_cn_k3_compiles_with_native_chat_and_auto_responses_bridge() {
    // Compile the complete registry so the Kimi Provider and model binding cross startup validation.
    let definition = compiled_config();
    let instance = definition
        .provider_instances
        .iter()
        .find(|instance| instance.id == "kimi-cn")
        .expect("Kimi CN Provider instance should be compiled");
    assert_eq!(instance.kind, ProviderKind::KimiCn);
    assert_eq!(instance.base_url, "https://api.moonshot.cn");

    let pool = definition
        .credential_pools
        .iter()
        .find(|pool| pool.id == "kimi-primary")
        .expect("Kimi API-key pool should be compiled");
    assert_eq!(pool.provider, ProviderKind::KimiCn);
    assert_eq!(pool.kind, CredentialKind::ApiKey);

    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let target = registry
        .upstream_target("kimi-cn-kimi-k3")
        .expect("Kimi K3 Target should compile");
    assert_eq!(target.kind(), ProviderKind::KimiCn);
    assert_eq!(target.canonical_model_id(), "moonshotai/kimi-k3");
    assert_eq!(target.provider_model_id(), "kimi-cn/kimi-k3");
    assert_eq!(target.endpoint_base().as_str(), "https://api.moonshot.cn/");
    assert_eq!(target.credential_pool_id(), "kimi-primary");
    assert_eq!(target.quota_scope(), Some("kimi-primary"));
    assert_eq!(target.fault_domain(), Some("kimi-cn-api"));
    assert_eq!(target.upstream_apis().count(), 1);
    assert_eq!(
        target
            .upstream_api(OperationKind::ChatCompletions)
            .expect("Kimi K3 Chat API should compile")
            .upstream_model(),
        "kimi-k3"
    );
    assert!(target.upstream_api(OperationKind::Responses).is_none());

    let public_model = registry
        .public_model("kimi-k3")
        .expect("Kimi K3 Public Model should compile");
    assert_eq!(
        public_model.routes(),
        ["kimi-k3-kimi-cn-chat", "kimi-k3-kimi-cn-responses-via-chat"]
    );
    let info = serde_json::to_value(public_model.info()).unwrap();
    assert!(info["interfaces"]["chat_completions"].is_object());
    assert!(info["interfaces"]["responses"].is_object());

    // Plan a text Chat request to the sole same-protocol Native candidate.
    let body = bytes::Bytes::from_static(
        br#"{"model":"kimi-k3","messages":[{"role":"user","content":"hello"}],"stream":true}"#,
    );
    let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(plan.candidates().len(), 1);
    assert_eq!(plan.candidates()[0].route_id(), "kimi-k3-kimi-cn-chat");
    assert_eq!(
        plan.candidates()[0].upstream_operation(),
        OperationKind::ChatCompletions
    );
    assert!(plan.candidates()[0].bridge().is_none());

    // Plan a text Responses request through the automatically supplemented Bridge candidate.
    let body = bytes::Bytes::from_static(br#"{"model":"kimi-k3","input":"hello","stream":true}"#);
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(plan.candidates().len(), 1);
    assert_eq!(
        plan.candidates()[0].route_id(),
        "kimi-k3-kimi-cn-responses-via-chat"
    );
    assert_eq!(
        plan.candidates()[0].upstream_operation(),
        OperationKind::ChatCompletions
    );
    assert!(plan.candidates()[0].bridge().is_some());

    // Verify the closed Provider adapter emits only the trusted relative Chat path and upstream model.
    let request = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        bytes::Bytes::from_static(br#"{"model":"kimi-k3","messages":[]}"#),
    );
    let upstream = ProviderAdapter::for_kind(ProviderKind::KimiCn)
        .prepare_request(&request, "kimi-k3")
        .unwrap();
    assert_eq!(upstream.relative_uri().to_string(), "/v1/chat/completions");
    let upstream_body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(upstream_body["model"], "kimi-k3");
}

#[test]
fn nvidia_and_bailian_models_compile_with_native_chat_and_auto_responses_bridges() {
    // Compile the complete registry so every model binding crosses the startup validation boundary.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let cases = [
        (
            "minimax-m3",
            "nvidia-minimax-m3",
            ProviderKind::Nvidia,
            "minimax/minimax-m3",
            "nvidia/minimax-m3",
            "minimaxai/minimax-m3",
            "nvidia-primary",
            "nvidia-api",
            "https://integrate.api.nvidia.com/v1/",
            "minimax-m3-nvidia-chat",
            "minimax-m3-nvidia-responses-via-chat",
        ),
        (
            "glm-5.2",
            "bailian-glm-5-2",
            ProviderKind::Bailian,
            "z-ai/glm-5.2",
            "bailian/glm-5.2",
            "glm-5.2",
            "bailian-primary",
            "bailian-api",
            "https://dashscope.aliyuncs.com/compatible-mode/v1/",
            "glm-5-2-bailian-chat",
            "glm-5-2-bailian-responses-via-chat",
        ),
        (
            "qwen3.7-plus",
            "bailian-qwen3-7-plus",
            ProviderKind::Bailian,
            "qwen/qwen3.7-plus",
            "bailian/qwen3.7-plus",
            "qwen3.7-plus",
            "bailian-primary",
            "bailian-api",
            "https://dashscope.aliyuncs.com/compatible-mode/v1/",
            "qwen3-7-plus-bailian-chat",
            "qwen3-7-plus-bailian-responses-via-chat",
        ),
        (
            "qwen3.7-max",
            "bailian-qwen3-7-max",
            ProviderKind::Bailian,
            "qwen/qwen3.7-max",
            "bailian/qwen3.7-max",
            "qwen3.7-max",
            "bailian-primary",
            "bailian-api",
            "https://dashscope.aliyuncs.com/compatible-mode/v1/",
            "qwen3-7-max-bailian-chat",
            "qwen3-7-max-bailian-responses-via-chat",
        ),
    ];

    // Verify each fixed Target exposes one Chat Native Route and one automatic Responses Bridge.
    for (
        public_name,
        target_id,
        provider,
        canonical_model,
        provider_model,
        upstream_model,
        credential_pool,
        fault_domain,
        endpoint_base,
        route_id,
        responses_route_id,
    ) in cases
    {
        let target = registry
            .upstream_target(target_id)
            .expect("NVIDIA or Bailian model Target should compile");
        assert_eq!(target.kind(), provider);
        assert_eq!(target.canonical_model_id(), canonical_model);
        assert_eq!(target.provider_model_id(), provider_model);
        assert_eq!(target.endpoint_base().as_str(), endpoint_base);
        assert_eq!(target.credential_pool_id(), credential_pool);
        assert_eq!(target.quota_scope(), Some(credential_pool));
        assert_eq!(target.fault_domain(), Some(fault_domain));
        assert_eq!(
            target
                .upstream_api(OperationKind::ChatCompletions)
                .expect("Chat Completions should be enabled")
                .upstream_model(),
            upstream_model
        );
        assert!(target.upstream_api(OperationKind::Responses).is_none());

        let public_model = registry
            .public_model(public_name)
            .expect("NVIDIA or Bailian Public Model should compile");
        assert_eq!(
            public_model.routes(),
            [route_id.to_owned(), responses_route_id.to_owned()]
        );
        let info = serde_json::to_value(public_model.info()).unwrap();
        assert!(info["interfaces"]["chat_completions"].is_object());
        assert!(info["interfaces"]["responses"].is_object());

        // Plan a text Chat request to the sole same-protocol Native candidate.
        let body = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","messages":[{{"role":"user","content":"hello"}}],"stream":true}}"#
        ));
        let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        assert_eq!(plan.candidates().len(), 1);
        assert_eq!(plan.candidates()[0].route_id(), route_id);
        assert_eq!(
            plan.candidates()[0].upstream_operation(),
            OperationKind::ChatCompletions
        );
        assert!(plan.candidates()[0].bridge().is_none());
    }
}

#[test]
fn bailian_qwen_models_compile_as_fixed_chat_targets() {
    // Compile the complete registry so each new canonical Qwen profile crosses target validation.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let cases = [
        (
            "bailian-qwen3-8-max",
            "qwen/qwen3.8-max",
            "bailian/qwen3.8-max",
            "qwen3.8-max",
        ),
        (
            "bailian-qwen-image-2-0-pro",
            "qwen/qwen-image-2.0-pro",
            "bailian/qwen-image-2.0-pro",
            "qwen-image-2.0-pro",
        ),
        (
            "bailian-qwen3-5-livetranslate-flash-realtime",
            "qwen/qwen3.5-livetranslate-flash-realtime",
            "bailian/qwen3.5-livetranslate-flash-realtime",
            "qwen3.5-livetranslate-flash-realtime",
        ),
        (
            "bailian-qwen3-6-27b",
            "qwen/qwen3.6-27b",
            "bailian/qwen3.6-27b",
            "qwen3.6-27b",
        ),
    ];

    // Verify every fixed Target preserves the provider identity and single Chat API binding.
    for (target_id, canonical_model, provider_model, upstream_model) in cases {
        let target = registry
            .upstream_target(target_id)
            .expect("Bailian Qwen Target should compile");
        assert_eq!(target.kind(), ProviderKind::Bailian);
        assert_eq!(target.canonical_model_id(), canonical_model);
        assert_eq!(target.provider_model_id(), provider_model);
        assert_eq!(
            target.endpoint_base().as_str(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/"
        );
        assert_eq!(target.credential_pool_id(), "bailian-primary");
        assert_eq!(target.quota_scope(), Some("bailian-primary"));
        assert_eq!(target.fault_domain(), Some("bailian-api"));
        assert_eq!(
            target
                .upstream_api(OperationKind::ChatCompletions)
                .expect("Bailian Qwen Chat API should compile")
                .upstream_model(),
            upstream_model
        );
        assert!(target.upstream_api(OperationKind::Responses).is_none());
    }
}

#[test]
fn bailian_deepseek_models_compile_as_chat_native_fallbacks() {
    // Compile the complete registry so the new Targets and fallback Routes cross startup validation.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Verify both Bailian Targets retain their canonical identity and fixed Chat-only deployment.
    for (target_id, canonical_model, provider_model, upstream_model) in [
        (
            "bailian-deepseek-v4-pro",
            "deepseek/deepseek-v4-pro",
            "bailian/deepseek-v4-pro",
            "deepseek-v4-pro",
        ),
        (
            "bailian-deepseek-v4-flash",
            "deepseek/deepseek-v4-flash",
            "bailian/deepseek-v4-flash",
            "deepseek-v4-flash-0731",
        ),
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("Bailian DeepSeek Target should compile");
        assert_eq!(target.kind(), ProviderKind::Bailian);
        assert_eq!(target.canonical_model_id(), canonical_model);
        assert_eq!(target.provider_model_id(), provider_model);
        assert_eq!(
            target.endpoint_base().as_str(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/"
        );
        assert_eq!(target.credential_pool_id(), "bailian-primary");
        assert_eq!(target.quota_scope(), Some("bailian-primary"));
        assert_eq!(target.fault_domain(), Some("bailian-api"));
        let chat = target
            .upstream_api(OperationKind::ChatCompletions)
            .expect("Bailian DeepSeek Chat API should compile");
        assert_eq!(chat.upstream_model(), upstream_model);
        assert!(target.upstream_api(OperationKind::Responses).is_none());
        let capabilities = match chat.capabilities() {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities,
            UpstreamApiCapabilities::Responses(_) => panic!("expected Chat capabilities"),
            UpstreamApiCapabilities::Embeddings(_) => panic!("expected Chat capabilities"),
        };
        assert!(capabilities.function_tools.is_none());
    }

    // Preserve existing Provider priority while appending Bailian to Chat planning only.
    for (public_name, expected_routes, expected_chat_routes) in [
        (
            "deepseek-v4-pro",
            vec![
                "deepseek-v4-pro-deepseek-chat",
                "deepseek-v4-pro-bailian-chat",
                "deepseek-v4-pro-deepseek-responses-via-chat",
                "deepseek-v4-pro-bailian-responses-via-chat",
            ],
            vec![
                "deepseek-v4-pro-deepseek-chat",
                "deepseek-v4-pro-bailian-chat",
            ],
        ),
        (
            "deepseek-v4-flash",
            vec![
                "deepseek-v4-flash-deepseek-chat",
                "deepseek-v4-flash-openrouter-chat",
                "deepseek-v4-flash-bailian-chat",
                "deepseek-v4-flash-deepseek-responses",
                "deepseek-v4-flash-openrouter-responses",
            ],
            vec![
                "deepseek-v4-flash-deepseek-chat",
                "deepseek-v4-flash-openrouter-chat",
                "deepseek-v4-flash-bailian-chat",
            ],
        ),
    ] {
        let public_model = registry
            .public_model(public_name)
            .expect("DeepSeek Public Model should compile");
        assert_eq!(public_model.routes(), expected_routes);
        let info = serde_json::to_value(public_model.info()).unwrap();
        assert_eq!(
            info["interfaces"]["chat_completions"]["tools"]["support"],
            "unsupported"
        );

        let body = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","messages":[{{"role":"user","content":"hello"}}]}}"#
        ));
        let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        assert_eq!(
            plan.candidates()
                .iter()
                .map(|candidate| candidate.route_id())
                .collect::<Vec<_>>(),
            expected_chat_routes
        );

        let tools = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","messages":[],"tools":[{{"type":"function","function":{{"name":"probe"}}}}]}}"#
        ));
        let profile = analyze_request(ApiProtocol::ChatCompletions, &tools).unwrap();
        assert!(matches!(
            plan_request(&registry, &profile, tools),
            Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
        ));
    }
}

#[test]
fn chatgpt_targets_are_compiled_as_oauth_responses_routes_with_chat_bridge() {
    // Locate the dedicated OAuth pool and fixed ChatGPT Provider instance.
    let definition = compiled_config();
    let pool = definition
        .credential_pools
        .iter()
        .find(|pool| pool.id == "chatgpt-codex")
        .expect("ChatGPT OAuth pool should be compiled");
    assert_eq!(pool.provider, ProviderKind::ChatGpt);
    assert_eq!(pool.kind, CredentialKind::OAuth2BearerAccessToken);
    let provider_instance = definition
        .provider_instances
        .iter()
        .find(|instance| instance.id == "chatgpt")
        .expect("ChatGPT Provider instance should be compiled");
    assert_eq!(provider_instance.kind, ProviderKind::ChatGpt);
    assert_eq!(
        provider_instance.base_url,
        "https://chatgpt.com/backend-api/codex"
    );

    // Verify the four ChatGPT-only Public Models expose one Chat bridge and one Responses-native Route.
    for (public_name, target_id, canonical_model, upstream_model, advanced_capabilities) in [
        (
            "chatgpt-gpt-5.3-codex-spark",
            "chatgpt-gpt-5-3-codex-spark",
            "chatgpt/gpt-5.3-codex-spark",
            "gpt-5.3-codex-spark",
            false,
        ),
        (
            "chatgpt-gpt-5.5",
            "chatgpt-gpt-5-5",
            "chatgpt/gpt-5.5",
            "gpt-5.5",
            true,
        ),
        (
            "chatgpt-gpt-5.6-luna",
            "chatgpt-gpt-5-6-luna",
            "chatgpt/gpt-5.6-luna",
            "gpt-5.6-luna",
            true,
        ),
        (
            "chatgpt-gpt-5.6-terra",
            "chatgpt-gpt-5-6-terra",
            "chatgpt/gpt-5.6-terra",
            "gpt-5.6-terra",
            true,
        ),
    ] {
        let target = definition
            .upstream_targets
            .iter()
            .find(|target| target.id == target_id)
            .expect("ChatGPT target should be compiled");
        assert_eq!(target.provider_instance, "chatgpt");
        assert_eq!(target.canonical_model, canonical_model);
        assert_eq!(
            target.provider_model,
            format!("chatgpt/{}", public_name.strip_prefix("chatgpt-").unwrap())
        );
        assert_eq!(target.credential_pool, "chatgpt-codex");
        assert!(target.enabled);
        assert_eq!(target.upstream_apis.len(), 1);
        assert_eq!(
            target.upstream_apis[0].capabilities.operation(),
            OperationKind::Responses
        );
        assert_eq!(target.upstream_apis[0].upstream_model, upstream_model);
        let responses_capabilities = match target.upstream_apis[0].capabilities {
            UpstreamApiCapabilities::Responses(capabilities) => capabilities,
            UpstreamApiCapabilities::ChatCompletions(_) => {
                panic!("expected ChatGPT Responses capabilities")
            }
            UpstreamApiCapabilities::Embeddings(_) => {
                panic!("expected ChatGPT generation capabilities")
            }
        };
        assert_eq!(
            responses_capabilities.function_tools.is_some(),
            advanced_capabilities
        );
        assert_eq!(
            responses_capabilities
                .function_tools
                .is_some_and(|profile| profile.parallel_calls),
            advanced_capabilities
        );
        assert_eq!(
            responses_capabilities.structured_outputs.is_some(),
            advanced_capabilities
        );

        let public_model = definition
            .public_models
            .iter()
            .find(|model| model.id == public_name)
            .expect("ChatGPT Public Model should be compiled");
        assert_eq!(
            public_model.routes,
            [
                format!("{target_id}-chat-via-responses"),
                format!("{target_id}-responses")
            ]
        );
        let chat_route = definition
            .routes
            .iter()
            .find(|route| route.id == public_model.routes[0])
            .expect("ChatGPT Chat bridge Route should be compiled");
        assert_eq!(chat_route.upstream_target, target_id);
        assert_eq!(chat_route.upstream_operation, OperationKind::Responses);
        assert_eq!(
            chat_route.downstream_operation,
            ApiProtocol::ChatCompletions.operation()
        );
        assert_eq!(chat_route.mode, RouteMode::Bridged);
        let responses_route = definition
            .routes
            .iter()
            .find(|route| route.id == public_model.routes[1])
            .expect("ChatGPT Responses Route should be compiled");
        assert_eq!(responses_route.upstream_target, target_id);
        assert_eq!(responses_route.upstream_operation, OperationKind::Responses);
        assert_eq!(
            responses_route.downstream_operation,
            OperationKind::Responses
        );
        assert_eq!(responses_route.mode, RouteMode::Native);
    }

    // Compile the runtime snapshot and prove each downstream protocol selects its fixed Route.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    for (public_name, target_id) in [
        ("chatgpt-gpt-5.3-codex-spark", "chatgpt-gpt-5-3-codex-spark"),
        ("chatgpt-gpt-5.5", "chatgpt-gpt-5-5"),
        ("chatgpt-gpt-5.6-luna", "chatgpt-gpt-5-6-luna"),
        ("chatgpt-gpt-5.6-terra", "chatgpt-gpt-5-6-terra"),
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("enabled ChatGPT target should compile");
        assert!(target.enabled());
        assert_eq!(target.kind(), ProviderKind::ChatGpt);

        let chat_body = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","messages":[{{"role":"user","content":"hello"}}],"stream":true}}"#
        ));
        let chat_profile = analyze_request(ApiProtocol::ChatCompletions, &chat_body).unwrap();
        let chat_plan = plan_request(&registry, &chat_profile, chat_body).unwrap();
        assert_eq!(chat_plan.candidates().len(), 1);
        assert_eq!(
            chat_plan.candidates()[0].route_id(),
            format!("{target_id}-chat-via-responses")
        );
        assert!(chat_plan.candidates()[0].bridge().is_some());

        let body = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","input":"hello","stream":true}}"#
        ));
        let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        assert_eq!(plan.candidates().len(), 1);
        assert_eq!(
            plan.candidates()[0].route_id(),
            format!("{target_id}-responses")
        );
        assert!(plan.candidates()[0].bridge().is_none());
    }

    // GPT-5.5 and GPT-5.6 expose the complete function-tool contract on both downstream surfaces.
    for public_name in [
        "chatgpt-gpt-5.5",
        "chatgpt-gpt-5.6-luna",
        "chatgpt-gpt-5.6-terra",
    ] {
        let info = serde_json::to_value(
            registry
                .public_model(public_name)
                .expect("ChatGPT advanced Public Model should compile")
                .info(),
        )
        .unwrap();
        for protocol in ["chat_completions", "responses"] {
            assert_eq!(
                info["interfaces"][protocol]["tools"]["support"],
                "supported"
            );
            assert_eq!(
                info["interfaces"][protocol]["tools"]["parallel_calls"],
                "supported"
            );
            assert_eq!(
                info["interfaces"][protocol]["structured_outputs"]["support"],
                "supported"
            );
        }
    }
}

#[test]
fn deepseek_models_preserve_primary_routes_with_bailian_chat_fallbacks() {
    // Build the complete compiled registry and check the fixed trusted boundaries of both DeepSeek targets.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
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
        assert_eq!(target.canonical_model_id(), canonical_model);
        assert_eq!(
            target.provider_model_id(),
            format!("deepseek/{}", canonical_model.rsplit_once('/').unwrap().1)
        );
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
            target
                .upstream_api(OperationKind::ChatCompletions)
                .unwrap()
                .upstream_model(),
            public_name
        );
        assert_eq!(
            target
                .upstream_api(OperationKind::ChatCompletions)
                .unwrap()
                .reasoning_output(),
            ReasoningOutput::PlainText
        );
        if public_name == "deepseek-v4-pro" {
            assert!(target.upstream_api(OperationKind::Responses).is_none());
        } else {
            let responses = target
                .upstream_api(OperationKind::Responses)
                .expect("DeepSeek V4 Flash Responses API should be compiled");
            assert_eq!(responses.upstream_model(), "deepseek-v4-flash");
            assert_eq!(responses.state_affinity(), StateAffinity::Unbound);
            assert_eq!(responses.reasoning_output(), ReasoningOutput::Unknown);
        }

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
            assert_eq!(
                public_model.routes(),
                [
                    "deepseek-v4-pro-deepseek-chat",
                    "deepseek-v4-pro-bailian-chat",
                    "deepseek-v4-pro-deepseek-responses-via-chat",
                    "deepseek-v4-pro-bailian-responses-via-chat"
                ]
            );
            assert!(info["interfaces"]["responses"].is_object());
        } else {
            // Flash appends Bailian to Chat while retaining the two existing Responses sources.
            assert_eq!(
                public_model.routes(),
                [
                    "deepseek-v4-flash-deepseek-chat",
                    "deepseek-v4-flash-openrouter-chat",
                    "deepseek-v4-flash-bailian-chat",
                    "deepseek-v4-flash-deepseek-responses",
                    "deepseek-v4-flash-openrouter-responses",
                ]
            );
            let responses =
                bytes::Bytes::from_static(br#"{"model":"deepseek-v4-flash","input":"hello"}"#);
            let profile = analyze_request(ApiProtocol::Responses, &responses).unwrap();
            let plan = plan_request(&registry, &profile, responses).unwrap();
            assert_eq!(
                plan.candidates()
                    .iter()
                    .map(|candidate| candidate.route_id())
                    .collect::<Vec<_>>(),
                [
                    "deepseek-v4-flash-deepseek-responses",
                    "deepseek-v4-flash-openrouter-responses"
                ]
            );
            assert!(info["interfaces"]["responses"].is_object());
        }
    }
}

#[test]
fn mimo_v25_native_responses_accepts_prior_reasoning_items() {
    // Build the complete compiled registry and exercise the Native-only MiMo Responses route.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");

    // Keep an existing reasoning item on the Native route without introducing a lossy Bridge candidate.
    let mimo_body = bytes::Bytes::from(
        r#"{"model":"mimo-v2.5","input":[{"type":"reasoning","id":"rs_1","summary":[{"type":"summary_text","text":"prior"}]}]}"#,
    );
    let mimo_profile = analyze_request(ApiProtocol::Responses, &mimo_body).unwrap();
    let mimo_plan = plan_request(&registry, &mimo_profile, mimo_body).unwrap();
    assert_eq!(mimo_plan.candidates().len(), 1);
    assert_eq!(
        mimo_plan.candidates()[0].route_id(),
        "mimo-v2-5-mimo-responses"
    );
    assert_eq!(
        mimo_plan.candidates()[0].upstream_operation(),
        OperationKind::Responses
    );
    assert!(mimo_plan.candidates()[0].bridge().is_none());
}

#[test]
fn mimo_v25_image_requests_use_only_same_protocol_native_routes() {
    // Compile the production catalog so image eligibility is evaluated from the complete fixed interfaces.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let cases = [
        (
            ApiProtocol::ChatCompletions,
            bytes::Bytes::from_static(
                br#"{"model":"mimo-v2.5","messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,iVBORw0KGgo="}},{"type":"text","text":"describe"}]}]}"#,
            ),
            "mimo-v2-5-mimo-chat",
            OperationKind::ChatCompletions,
        ),
        (
            ApiProtocol::Responses,
            bytes::Bytes::from_static(
                br#"{"model":"mimo-v2.5","input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/png;base64,iVBORw0KGgo="},{"type":"input_text","text":"describe"}]}]}"#,
            ),
            "mimo-v2-5-mimo-responses",
            OperationKind::Responses,
        ),
    ];

    // Require each image request to bind exactly one Native endpoint without a reverse-Bridge candidate.
    for (protocol, body, expected_route, expected_operation) in cases {
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body).unwrap();
        assert_eq!(plan.candidates().len(), 1);
        assert_eq!(plan.candidates()[0].route_id(), expected_route);
        assert_eq!(
            plan.candidates()[0].upstream_operation(),
            expected_operation
        );
        assert!(plan.candidates()[0].bridge().is_none());
    }
}

#[test]
fn mimo_models_compile_model_specific_native_and_bridge_surfaces() {
    // Build the complete compiled registry and check the fixed trusted boundaries of both MiMo targets.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    for (public_name, target_id, canonical_model, route_prefix, supports_images, has_bridges) in [
        (
            "mimo-v2.5-pro",
            "mimo-v2-5-pro",
            "xiaomi/mimo-v2.5-pro",
            "mimo-v2-5-pro-mimo",
            false,
            true,
        ),
        (
            "mimo-v2.5",
            "mimo-v2-5",
            "xiaomi/mimo-v2.5",
            "mimo-v2-5-mimo",
            true,
            false,
        ),
    ] {
        let target = registry
            .upstream_target(target_id)
            .expect("MiMo target should be compiled");
        assert_eq!(target.kind(), ProviderKind::MiMo);
        assert_eq!(target.canonical_model_id(), canonical_model);
        assert_eq!(
            target.provider_model_id(),
            format!("mimo/{}", canonical_model.rsplit_once('/').unwrap().1)
        );
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
            target
                .upstream_api(OperationKind::ChatCompletions)
                .unwrap()
                .upstream_model(),
            public_name
        );
        assert_eq!(
            target
                .upstream_api(OperationKind::Responses)
                .unwrap()
                .upstream_model(),
            public_name
        );
        assert_eq!(
            target
                .upstream_api(OperationKind::ChatCompletions)
                .unwrap()
                .reasoning_output(),
            ReasoningOutput::Unknown
        );
        assert_eq!(
            target
                .upstream_api(OperationKind::Responses)
                .unwrap()
                .reasoning_output(),
            ReasoningOutput::Unknown
        );

        // Verify that both compiled Native APIs share the MiMo Provider contract's capability boundary.
        let chat_capabilities = match target
            .upstream_api(OperationKind::ChatCompletions)
            .unwrap()
            .capabilities()
        {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities,
            UpstreamApiCapabilities::Responses(_) => panic!("expected Chat capabilities"),
            UpstreamApiCapabilities::Embeddings(_) => panic!("expected Chat capabilities"),
        };
        assert!(
            chat_capabilities
                .function_tools
                .is_some_and(|profile| profile.parallel_calls)
        );
        assert_eq!(chat_capabilities.image_input.is_some(), supports_images);
        assert!(chat_capabilities.structured_outputs.is_some());
        assert!(!chat_capabilities.store);
        let responses_capabilities = match target
            .upstream_api(OperationKind::Responses)
            .unwrap()
            .capabilities()
        {
            UpstreamApiCapabilities::Responses(capabilities) => capabilities,
            UpstreamApiCapabilities::ChatCompletions(_) => {
                panic!("expected Responses capabilities")
            }
            UpstreamApiCapabilities::Embeddings(_) => {
                panic!("expected Responses capabilities")
            }
        };
        assert!(
            responses_capabilities
                .function_tools
                .is_some_and(|profile| profile.parallel_calls)
        );
        assert_eq!(
            responses_capabilities.image_input.is_some(),
            supports_images
        );
        assert!(responses_capabilities.structured_outputs.is_some());
        assert!(!responses_capabilities.store);
        assert!(!responses_capabilities.previous_response_id);
        assert!(!responses_capabilities.background);

        // Verify the model-specific Native and reverse-Bridge route surfaces.
        let public_model = registry
            .public_model(public_name)
            .expect("MiMo Public Model should be compiled");
        let expected_routes = if has_bridges {
            vec![
                format!("{route_prefix}-chat"),
                format!("{route_prefix}-chat-via-responses"),
                format!("{route_prefix}-responses"),
                format!("{route_prefix}-responses-via-chat"),
            ]
        } else {
            vec![
                format!("{route_prefix}-chat"),
                format!("{route_prefix}-responses"),
            ]
        };
        assert_eq!(public_model.routes(), expected_routes);
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
            assert_eq!(plan.candidates().len(), if has_bridges { 2 } else { 1 });
        }

        // Function tools remain available on every compiled candidate.
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
            assert_eq!(plan.candidates().len(), if has_bridges { 2 } else { 1 });
        }

        // Image input is admitted only for mimo-v2.5 and remains Native-only.
        for (protocol, body) in [
            (
                ApiProtocol::ChatCompletions,
                format!(
                    r#"{{"model":"{public_name}","messages":[{{"role":"user","content":[{{"type":"image_url","image_url":{{"url":"https://example.invalid/image.png"}}}}]}}]}}"#
                ),
            ),
            (
                ApiProtocol::Responses,
                format!(
                    r#"{{"model":"{public_name}","input":[{{"role":"user","content":[{{"type":"input_image","image_url":"https://example.invalid/image.png"}}]}}]}}"#
                ),
            ),
        ] {
            let body = bytes::Bytes::from(body);
            let profile = analyze_request(protocol, &body).unwrap();
            let plan = plan_request(&registry, &profile, body);
            if supports_images {
                let plan = plan.expect("mimo-v2.5 image input should use its Native route");
                assert_eq!(plan.candidates().len(), 1);
                assert!(plan.candidates()[0].bridge().is_none());
            } else {
                assert!(matches!(
                    plan,
                    Err(openbridge::pipeline::RequestPlanningError::UnsupportedCapabilities)
                ));
            }
        }

        // Structured output is now shared by the modeled Native and reverse-Bridge paths.
        for (protocol, body, expected_route) in [
            (
                ApiProtocol::ChatCompletions,
                format!(
                    r#"{{"model":"{public_name}","messages":[],"response_format":{{"type":"json_schema","json_schema":{{"name":"answer","schema":{{"type":"object"}}}}}}}}"#
                ),
                format!("{route_prefix}-chat"),
            ),
            (
                ApiProtocol::Responses,
                format!(
                    r#"{{"model":"{public_name}","input":"return json","text":{{"format":{{"type":"json_schema","name":"answer","schema":{{"type":"object"}}}}}}}}"#
                ),
                format!("{route_prefix}-responses"),
            ),
        ] {
            let body = bytes::Bytes::from(body);
            let profile = analyze_request(protocol, &body).unwrap();
            let plan = plan_request(&registry, &profile, body).unwrap();
            assert_eq!(plan.candidates()[0].route_id(), expected_route);
            assert_eq!(plan.candidates().len(), if has_bridges { 2 } else { 1 });
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
