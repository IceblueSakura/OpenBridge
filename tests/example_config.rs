//! 验证仓库示例配置、编译模型目录和默认 route 事实保持一致。

use openbridge::{
    config::parse_bootstrap_config,
    core::{ApiProtocol, ReasoningOutput},
    identity::UserConfigPath,
    pipeline::{analyze_request, plan_request},
    provider::ProviderKind,
    providers::{build_compiled_registry, compiled_config},
    registry::{ReasoningLevel, ReasoningSupport, UpstreamApiCapabilities, build_registry},
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

    // 家族和版本模块迁移不得改变 canonical 模型目录的内容或稳定顺序。
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
    assert_eq!(longcat.context_length.context_tokens(), Some(1_048_756));
    assert_eq!(longcat.context_length.input_tokens(), None);
    assert_eq!(longcat.context_length.output_tokens(), Some(262_144));

    let sol = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.6-sol")
        .unwrap();
    assert_eq!(sol.context_length.context_tokens(), Some(1_050_000));
    assert_eq!(sol.context_length.input_tokens(), None);
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

    // 代表性模型保留 context、输出上限和标准 reasoning level。
    let deepseek = definition
        .models
        .iter()
        .find(|model| model.id == "deepseek/deepseek-v4-pro")
        .unwrap();
    assert_eq!(deepseek.context_length.context_tokens(), Some(1_048_576));
    assert_eq!(deepseek.context_length.input_tokens(), None);
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
    assert_eq!(nemotron.context_length.context_tokens(), Some(512_288));
    assert_eq!(nemotron.context_length.input_tokens(), None);
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
    assert_eq!(chat.model().context_length().input_tokens(), None);
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
    assert_eq!(openrouter.credential_pool_id(), "openrouter-primary");
    assert!(registry.credential_pool("openrouter-primary").is_some());
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
        br#"{"model":"nemotron-3-ultra","messages":[],"reasoning_effort":"high","tools":[{"type":"function","function":{"name":"probe"}}]}"#,
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
            r#"{"model":"LongCat-2.0","input":"hello","tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}]}"#,
        ),
    ] {
        let body = bytes::Bytes::copy_from_slice(body.as_bytes());
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body)
            .expect("LongCat should remain on the native path for both protocols");
        assert_eq!(plan.upstream_target_id(), "longcat-2");
    }

    // 反向 Bridge 的 reasoning 输出证据未知，因此固定 Responses 契约不能只选择 Native 放行。
    let body = bytes::Bytes::from(r#"{"model":"LongCat-2.0","input":"hello","reasoning":{}}"#);
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    assert!(matches!(
        plan_request(&registry, &profile, body),
        Err(openbridge::pipeline::RequestPlanningError::ReasoningUnsupported)
    ));
}

#[test]
fn deepseek_models_are_compiled_with_chat_native_and_responses_bridge_routes() {
    // 构造完整编译注册表并核对两个 DeepSeek target 的固定受信边界。
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

        // 验证下游 Chat 走 Native，Responses 只走显式 Chat bridge。
        let public_model = registry
            .public_model(public_name)
            .expect("DeepSeek Public Model should be compiled");
        let prefix = format!("{public_name}-deepseek");
        assert_eq!(
            public_model.routes(),
            [
                format!("{prefix}-chat"),
                format!("{prefix}-responses-via-chat"),
            ]
        );
        let chat = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","messages":[{{"role":"user","content":"hello"}}]}}"#
        ));
        let profile = analyze_request(ApiProtocol::ChatCompletions, &chat).unwrap();
        let plan = plan_request(&registry, &profile, chat).unwrap();
        assert_eq!(plan.candidates()[0].route_id(), format!("{prefix}-chat"));
        let responses =
            bytes::Bytes::from(format!(r#"{{"model":"{public_name}","input":"hello"}}"#));
        let profile = analyze_request(ApiProtocol::Responses, &responses).unwrap();
        let plan = plan_request(&registry, &profile, responses).unwrap();
        assert_eq!(
            plan.candidates()[0].route_id(),
            format!("{prefix}-responses-via-chat")
        );

        // 验证 DeepSeek Responses bridge 会在真实 route planning 后保留 reasoning effort。
        let responses_reasoning = bytes::Bytes::from(format!(
            r#"{{"model":"{public_name}","input":"hello","reasoning":{{"effort":"high"}}}}"#
        ));
        let profile = analyze_request(ApiProtocol::Responses, &responses_reasoning).unwrap();
        let plan = plan_request(&registry, &profile, responses_reasoning).unwrap();
        let upstream: serde_json::Value =
            serde_json::from_slice(plan.request().body()).expect("bridge request must be JSON");
        assert_eq!(upstream["reasoning_effort"], "high");
    }
}

#[test]
fn compiled_reasoning_output_types_match_deepseek_flash_and_mimo_v25_routes() {
    // 构造完整编译注册表，读取两个具体模型的实际 target 与 Provider API 分类。
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

    // DeepSeek Responses 只能选择 Chat Bridge，并使用已确认的 PlainText 上游 channel。
    let deepseek_body =
        bytes::Bytes::from(r#"{"model":"deepseek-v4-flash","input":"hello","reasoning":{}}"#);
    let deepseek_profile = analyze_request(ApiProtocol::Responses, &deepseek_body).unwrap();
    let deepseek_plan = plan_request(&registry, &deepseek_profile, deepseek_body).unwrap();
    assert_eq!(
        deepseek_plan.candidates()[0].route_id(),
        "deepseek-v4-flash-deepseek-responses-via-chat"
    );
    assert_eq!(deepseek_plan.candidates()[0].upstream_api_id(), "chat");
    assert!(deepseek_plan.candidates()[0].bridge().is_some());

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

    // MiMo 的 Bridge 无法安全表示既有 reasoning item，固定契约不能为此跳过 Bridge 候选。
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
    // 构造完整编译注册表并核对两个 MiMo target 的固定受信边界。
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

        // 验证编译后的两个 Native API 与 MiMo Provider contract 保持同一能力边界。
        let chat_capabilities = match target.upstream_api("chat").unwrap().capabilities() {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities,
            UpstreamApiCapabilities::Responses(_) => panic!("expected Chat capabilities"),
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
        };
        assert!(responses_capabilities.parallel_tool_calls);
        assert!(responses_capabilities.image_input);
        assert!(responses_capabilities.structured_outputs);
        assert!(!responses_capabilities.store);
        assert!(!responses_capabilities.previous_response_id);
        assert!(!responses_capabilities.background);

        // 验证两种下游协议都按 Native-first、反向 Bridge-second 排序。
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

        // function tools 属于两个完整 Route 的共同能力，保持 Native-first 和 Bridge fallback。
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

        // image 与 structured output 不是反向 Bridge 的完整共同语义，因此固定契约统一拒绝。
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

        // 验证 MiMo 的无状态边界仍拒绝 stateful Responses 请求。
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
    // 构造完整注册表，并从无真实值的 TOML 模板加载全部 credential pool。
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

    // 验证每个 target 可按 Provider 与 pool 取回模板中的合成凭证。
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
    alternate.credential_pool = "openai-primary".to_owned();
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
