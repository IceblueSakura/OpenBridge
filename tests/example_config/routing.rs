//! Verifies compiled Bridge selection and multi-Provider route ordering.

use super::*;

#[test]
fn compiled_registry_can_select_each_protocol_bridge_when_the_native_api_is_unavailable() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
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
    definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "chatgpt-gpt-5-6-sol")
        .expect("the ChatGPT pool member must exist")
        .enabled = false;
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
    let bootstrap = include_str!("../../config/bootstrap.toml");
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
    alternate.provider_instance = "openai".to_owned();
    alternate.provider_model = "openai/longcat-2.0".to_owned();
    alternate.credential_pool = "openai-primary".to_owned();
    for upstream_api in &mut alternate.upstream_apis {
        upstream_api.upstream_model = "longcat/longcat-2.0".to_owned();
        match &mut upstream_api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.function_tools = None;
                capabilities.reasoning_output = ReasoningOutput::Unknown;
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.function_tools = None;
                capabilities.reasoning_output = ReasoningOutput::Unknown;
            }
            UpstreamApiCapabilities::Embeddings(_) => {
                panic!("generation target must not contain Embeddings capabilities")
            }
        }
    }
    definition.upstream_targets.push(alternate);

    // Add the alternate Provider's complete surface and aggregate both targets Native-first per protocol.
    definition.routes.extend([
        RouteConfig {
            id: "longcat-openai-chat".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: ApiProtocol::ChatCompletions.operation(),
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "longcat-openai-chat-via-responses".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: ApiProtocol::ChatCompletions.operation(),
            mode: RouteMode::Bridged,
        },
        RouteConfig {
            id: "longcat-openai-responses".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: ApiProtocol::Responses.operation(),
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "longcat-openai-responses-via-chat".to_owned(),
            upstream_target: "openai-longcat-test".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
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
        .upstream_api(OperationKind::ChatCompletions)
        .unwrap();
    let alternate = registry
        .upstream_target("openai-longcat-test")
        .expect("alternate provider target exists")
        .upstream_api(OperationKind::ChatCompletions)
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
