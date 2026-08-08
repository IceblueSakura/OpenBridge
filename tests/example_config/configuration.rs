//! Verifies checked-in bootstrap examples, credential pools, and compiled registry consistency.

use super::*;

#[test]
fn checked_in_bootstrap_and_compiled_registry_are_loadable() {
    let bootstrap = include_str!("../../config/bootstrap.toml");
    let bootstrap =
        parse_bootstrap_config(bootstrap).expect("checked-in bootstrap must remain valid");
    let bootstrap_template = include_str!("../../config/bootstrap.example.toml");
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
            "gpt-5.6-sol-chatgpt-chat-via-responses",
            "gpt-5.6-sol-openai-responses",
            "gpt-5.6-sol-chatgpt-responses",
            "gpt-5.6-sol-openai-responses-via-chat",
        ]
    );
    assert!(registry.public_model("openai/gpt-5.6-sol").is_none());
    assert!(registry.public_model("chatgpt/gpt-5.6-sol").is_none());
    let gpt_pool = registry
        .public_model("gpt-5.6-sol")
        .expect("the merged GPT-5.6 Sol Public Model is compiled");
    let gpt_info = serde_json::to_value(gpt_pool.info()).unwrap();
    assert_eq!(
        gpt_info["interfaces"]["chat_completions"]["context_window"]["max_context_tokens"],
        272_000
    );
    assert_eq!(
        gpt_info["interfaces"]["responses"]["context_window"]["max_context_tokens"],
        272_000
    );

    let longcat = registry
        .public_model("LongCat-2.0")
        .expect("LongCat public model is compiled");
    assert_eq!(longcat.routes().len(), 4);
    let target = registry
        .upstream_target("longcat-2")
        .expect("LongCat target is compiled");
    let chat = target.upstream_api(OperationKind::ChatCompletions).unwrap();
    assert_eq!(target.kind(), ProviderKind::LongCat);
    assert_eq!(chat.upstream_model(), "LongCat-2.0");
    assert_eq!(chat.reasoning_output(), ReasoningOutput::Unknown);
    assert_eq!(
        target
            .upstream_api(OperationKind::Responses)
            .unwrap()
            .reasoning_output(),
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
        openai
            .upstream_api(OperationKind::ChatCompletions)
            .unwrap()
            .model()
            .id(),
        "openai/gpt-5.6-sol"
    );
    assert_eq!(
        openai
            .upstream_api(OperationKind::ChatCompletions)
            .unwrap()
            .upstream_model(),
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
            "deepseek-v4-flash-deepseek-responses",
            "deepseek-v4-flash-openrouter-responses"
        ]
    );
    let openrouter = registry
        .upstream_target("openrouter-deepseek-v4-flash")
        .expect("OpenRouter DeepSeek V4 Flash target is compiled");
    assert_eq!(openrouter.kind(), ProviderKind::OpenRouter);
    assert_eq!(
        openrouter.canonical_model_id(),
        "deepseek/deepseek-v4-flash"
    );
    assert_eq!(
        openrouter.provider_model_id(),
        "openrouter/deepseek-v4-flash"
    );
    assert_eq!(openrouter.credential_pool_id(), "openrouter-primary");
    assert!(registry.credential_pool("openrouter-primary").is_some());
    assert_eq!(
        openrouter.endpoint_base().as_str(),
        "https://openrouter.ai/api/v1/"
    );
    let openrouter_chat = openrouter
        .upstream_api(OperationKind::ChatCompletions)
        .unwrap();
    assert_eq!(
        openrouter_chat.upstream_model(),
        "deepseek/deepseek-v4-flash"
    );
    let openrouter_responses = openrouter.upstream_api(OperationKind::Responses).unwrap();
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
        plan.candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        [
            "deepseek-v4-flash-deepseek-responses",
            "deepseek-v4-flash-openrouter-responses"
        ]
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
