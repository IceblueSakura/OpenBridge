use openbridge::{
    config::parse_bootstrap_config,
    core::ApiProtocol,
    identity::UserConfigPath,
    pipeline::{analyze_request, plan_request},
    provider::ProviderKind,
    providers::{build_compiled_registry, compiled_config},
    registry::{ReasoningLevel, ReasoningSupport, UpstreamApiCapabilities, build_registry},
};

#[test]
fn compiled_model_catalog_includes_litellm_text_models() {
    let definition = compiled_config();
    let expected = [
        "openai/gpt-5.6-sol",
        "openai/gpt-5.6-terra",
        "openai/gpt-5.6-luna",
        "openai/gpt-5.5",
        "openai/gpt-5.3-codex-spark",
        "deepseek/deepseek-v4-pro",
        "deepseek/deepseek-v4-flash",
        "xiaomi/mimo-v2.5-pro",
        "xiaomi/mimo-v2.5",
        "qwen/qwen3.7-max",
        "qwen/qwen3.7-plus",
        "z-ai/glm-5.2",
        "moonshotai/kimi-k3",
        "minimax/minimax-m3",
        "tencent/hy3",
        "nvidia/nemotron-3-ultra-550b-a55b",
    ];

    // 每个 LiteLLM Chat/Responses 模型组只产生一个 canonical 模型定义。
    for id in expected {
        assert!(
            definition.models.iter().any(|model| model.id == id),
            "missing canonical model {id}"
        );
    }
    assert_eq!(definition.models.len(), expected.len() + 1);
    assert!(
        definition
            .models
            .iter()
            .all(|model| model.id != "openai/configured-model")
    );

    // 除 OpenRouter 未精确收录的 Codex Spark 外，每个模型都有官方目录描述。
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
    assert_eq!(longcat.context_length.input_tokens(), Some(1_048_756));
    assert_eq!(longcat.context_length.output_tokens(), Some(262_144));

    let sol = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.6-sol")
        .unwrap();
    assert_eq!(sol.context_length.input_tokens(), Some(1_050_000));
    assert_eq!(sol.context_length.output_tokens(), Some(128_000));
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
    assert_eq!(codex_spark.context_length.input_tokens(), Some(128_000));
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

    // 代表性模型保留 context、输出上限和标准 reasoning level。
    let deepseek = definition
        .models
        .iter()
        .find(|model| model.id == "deepseek/deepseek-v4-pro")
        .unwrap();
    assert_eq!(deepseek.context_length.input_tokens(), Some(1_048_576));
    assert_eq!(deepseek.context_length.output_tokens(), Some(384_000));
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

    let hy3 = definition
        .models
        .iter()
        .find(|model| model.id == "tencent/hy3")
        .unwrap();
    assert_eq!(
        hy3.reasoning_levels,
        [
            ReasoningLevel::High,
            ReasoningLevel::Low,
            ReasoningLevel::None
        ]
    );

    let nemotron = definition
        .models
        .iter()
        .find(|model| model.id == "nvidia/nemotron-3-ultra-550b-a55b")
        .unwrap();
    assert_eq!(nemotron.context_length.input_tokens(), Some(512_288));
    assert_eq!(nemotron.context_length.output_tokens(), None);
    assert!(
        nemotron
            .supported_parameters
            .iter()
            .any(|parameter| parameter == "structured_outputs")
    );

    // 当前目录类型不表示 embedding/rerank，避免把不可路由协议伪装为文本模型。
    assert!(
        definition
            .models
            .iter()
            .all(|model| !model.id.contains("embed") && !model.id.contains("rerank"))
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
            .public_model("code-primary")
            .expect("public model is compiled")
            .routes(),
        [
            "code-primary-openai-chat",
            "code-primary-openai-chat-via-responses",
            "code-primary-openai-responses",
            "code-primary-openai-responses-via-chat",
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
    assert_eq!(target.endpoint_base().as_str(), "https://api.longcat.chat/");
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
        .public_model("nemotron-3-ultra")
        .expect("OpenRouter Nemotron public model is compiled");
    assert_eq!(
        openrouter_public.routes(),
        [
            "nemotron-3-ultra-openrouter-chat",
            "nemotron-3-ultra-openrouter-responses"
        ]
    );
    let openrouter = registry
        .upstream_target("openrouter-nemotron-3-ultra")
        .expect("OpenRouter Nemotron target is compiled");
    assert_eq!(openrouter.kind(), ProviderKind::OpenRouter);
    assert_eq!(openrouter.model_id(), "nvidia/nemotron-3-ultra-550b-a55b");
    assert_eq!(openrouter.credential().id(), "openrouter-primary");
    assert_eq!(
        openrouter.credential().secret_reference().locator(),
        "OPENROUTER_API_KEY"
    );
    assert_eq!(
        openrouter.endpoint_base().as_str(),
        "https://openrouter.ai/api/v1/"
    );
    let openrouter_chat = openrouter.upstream_api("chat").unwrap();
    assert_eq!(
        openrouter_chat.upstream_model(),
        "nvidia/nemotron-3-ultra-550b-a55b"
    );
    let openrouter_responses = openrouter.upstream_api("responses").unwrap();
    assert_eq!(
        openrouter_responses.upstream_model(),
        "nvidia/nemotron-3-ultra-550b-a55b"
    );
    let responses_capabilities = match openrouter_responses.capabilities() {
        UpstreamApiCapabilities::Responses(capabilities) => capabilities,
        UpstreamApiCapabilities::ChatCompletions(_) => panic!("expected Responses capabilities"),
    };
    assert!(responses_capabilities.enabled);
    assert!(responses_capabilities.streaming);
    assert!(responses_capabilities.function_calling);
    assert!(!responses_capabilities.store);
    assert!(!responses_capabilities.previous_response_id);
    assert!(!responses_capabilities.background);

    let body = bytes::Bytes::from_static(
        br#"{"model":"nemotron-3-ultra","messages":[],"reasoning":{"effort":"high"},"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
    );
    let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(
        plan.candidates()[0].route_id(),
        "nemotron-3-ultra-openrouter-chat"
    );
    assert_eq!(
        plan.candidates()[0].upstream_target_id(),
        "openrouter-nemotron-3-ultra"
    );

    let responses = bytes::Bytes::from_static(
        br#"{"model":"nemotron-3-ultra","input":"hello","stream":true,"reasoning":{"effort":"high"},"tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}]}"#,
    );
    let profile = analyze_request(ApiProtocol::Responses, &responses).unwrap();
    let plan = plan_request(&registry, &profile, responses).unwrap();
    assert_eq!(
        plan.candidates()[0].route_id(),
        "nemotron-3-ultra-openrouter-responses"
    );

    for unsupported in [
        br#"{"model":"nemotron-3-ultra","input":"hello","store":true}"#.as_slice(),
        br#"{"model":"nemotron-3-ultra","input":"hello","previous_response_id":"resp_123"}"#
            .as_slice(),
        br#"{"model":"nemotron-3-ultra","input":"hello","background":true}"#.as_slice(),
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
            r#"{"model":"LongCat-2.0","input":"hello","tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}],"reasoning":{}}"#,
        ),
    ] {
        let body = bytes::Bytes::copy_from_slice(body.as_bytes());
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body)
            .expect("LongCat should remain on the native path for both protocols");
        assert_eq!(plan.upstream_target_id(), "longcat-2");
    }
}

#[test]
fn unconnected_provider_definitions_are_absent_from_the_compiled_registry() {
    let definition = compiled_config();

    assert!(
        definition
            .upstream_targets
            .iter()
            .all(|target| !matches!(target.provider, ProviderKind::DeepSeek | ProviderKind::MiMo))
    );
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

    // 关闭 Chat native capability，Chat 下游请求必须改走 Responses bridge。
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut target.upstream_apis[0].capabilities
    {
        capabilities.enabled = false;
    }
    let registry = build_registry(bootstrap.clone(), definition.clone()).unwrap();
    let body = bytes::Bytes::from_static(
        br#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}"#,
    );
    let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(
        plan.candidates()[0].route_id(),
        "code-primary-openai-chat-via-responses"
    );
    assert!(plan.candidates()[0].bridge().is_some());

    // 反向关闭 Responses native capability，Responses 下游请求必须改走 Chat bridge。
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
    let body = bytes::Bytes::from_static(br#"{"model":"code-primary","input":"hello"}"#);
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(
        plan.candidates()[0].route_id(),
        "code-primary-openai-responses-via-chat"
    );
    assert!(plan.candidates()[0].bridge().is_some());
}

#[test]
fn real_model_can_be_shared_by_targets_from_different_providers() {
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
    alternate.credential.id = "openai-longcat-test".to_owned();
    alternate.credential.environment_variable = "OPENAI_API_KEY".to_owned();
    for upstream_api in &mut alternate.upstream_apis {
        upstream_api.upstream_model = "longcat/longcat-2.0".to_owned();
        upstream_api.endpoint_profile = "public-api".to_owned();
    }
    alternate.base_url = "https://api.openai.com".to_owned();
    definition.upstream_targets.push(alternate);

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
}
