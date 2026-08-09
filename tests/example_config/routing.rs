//! Verifies compiled Bridge selection and multi-Provider route ordering.

use super::*;

#[test]
fn subscription_only_gpt_chat_uses_the_responses_bridge() {
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let mut definition = compiled_config();

    // Reproduce the subscription-only deployment used by the real end-to-end matrix.
    definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "openai-main")
        .expect("the OpenAI source must exist")
        .enabled = false;
    let registry = build_registry(bootstrap, definition).unwrap();

    // Require the remaining Responses-only source to provide an explicit streaming Chat Bridge.
    let body = bytes::Bytes::from_static(
        br#"{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"hello"}],"stream":true}"#,
    );
    let profile = analyze_request(ApiProtocol::ChatCompletions, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    let [candidate] = plan.candidates() else {
        panic!("the subscription-only deployment must have one Chat candidate");
    };
    let target = registry
        .upstream_target(candidate.upstream_target_id())
        .expect("planned Target must resolve");
    assert_eq!(target.kind(), ProviderKind::ChatGpt);
    assert_eq!(candidate.upstream_operation(), OperationKind::Responses);
    assert!(candidate.bridge().is_some());
}

#[test]
fn longcat_responses_tool_continuation_prepares_native_and_bridge_candidates() {
    // Compile the checked-in dual-protocol LongCat source with its Native-first route order.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_registry(bootstrap, compiled_config()).unwrap();
    let body = bytes::Bytes::from_static(
        br#"{"model":"LongCat-2.0","input":[{"role":"user","content":"look up the synthetic value"},{"type":"function_call","id":"fc_lookup","call_id":"call_lookup","name":"lookup","arguments":"{\"key\":\"value\"}"},{"type":"function_call_output","call_id":"call_lookup","output":"{\"value\":42}"},{"role":"user","content":"return DONE"}],"tools":[{"name":"lookup","parameters":{"type":"object"},"type":"function"}],"tool_choice":"none"}"#,
    );

    // Require every fixed candidate to prepare so a convertible fallback cannot invalidate the Native primary.
    let profile = analyze_request(ApiProtocol::Responses, &body).unwrap();
    let plan = plan_request(&registry, &profile, body).unwrap();
    assert_eq!(
        plan.candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        ["longcat-2-responses", "longcat-2-responses-via-chat"]
    );
    assert!(plan.candidates()[0].bridge().is_none());
    let bridge_candidate = &plan.candidates()[1];
    assert!(bridge_candidate.bridge().is_some());
    assert_eq!(
        bridge_candidate.request().protocol(),
        ApiProtocol::ChatCompletions
    );
    let chat_request: serde_json::Value =
        serde_json::from_slice(bridge_candidate.request().body()).unwrap();
    assert_eq!(chat_request["messages"].as_array().unwrap().len(), 4);
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
