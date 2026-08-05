//! Verifies Public Model capability preflight and ordered Route planning.

mod support;

use openbridge::{
    core::{ApiProtocol, ReasoningOutput},
    pipeline::RequestPlanningError,
    registry::{
        ModelContextLength, ReasoningLevel, ReasoningLevelMapping, ReasoningSupport,
        RegistryConfig, RouteConfig, RouteMode, RuntimeRegistry, build_registry,
    },
};
use serde_json::{Value, json};

fn base_definition() -> RegistryConfig {
    support::definition("routing-test", "public-model", "upstream-model")
}

#[test]
fn planning_preserves_canonical_reasoning_levels_for_every_candidate() {
    let mut definition = base_definition();
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::XHigh];
    for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
        upstream_api.model_rules.reasoning_level_mappings = vec![ReasoningLevelMapping {
            downstream: ReasoningLevel::XHigh,
            upstream: "max".to_owned(),
        }];
    }
    let mut unmapped = definition.upstream_targets[0].clone();
    unmapped.id = "openai-unmapped".to_owned();
    for upstream_api in &mut unmapped.upstream_apis {
        upstream_api.model_rules.reasoning_level_mappings.clear();
    }
    definition.upstream_targets.push(unmapped);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "unmapped-chat".to_owned(),
        upstream_target: "openai-unmapped".to_owned(),
        upstream_api: "chat".to_owned(),
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "unmapped-responses".to_owned(),
        upstream_target: "openai-unmapped".to_owned(),
        upstream_api: "responses".to_owned(),
        downstream_operation: ApiProtocol::Responses.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0].routes = vec![
        "public-chat".to_owned(),
        "unmapped-chat".to_owned(),
        "public-responses".to_owned(),
        "unmapped-responses".to_owned(),
    ];
    let registry = build_test_registry(definition);

    // Verify that planning preserves the canonical Responses level for every candidate.
    let request = json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "xhigh"}
    });
    let prepared = support::prepare(
        &registry,
        ApiProtocol::Responses,
        serde_json::to_vec(&request).unwrap().into(),
    )
    .unwrap();
    let mapped: Value = serde_json::from_slice(prepared.request().body()).unwrap();
    assert_eq!(mapped["reasoning"]["effort"], "xhigh");
    assert_eq!(prepared.candidates().len(), 2);
    let unmapped: Value =
        serde_json::from_slice(prepared.candidates()[1].request().body()).unwrap();
    assert_eq!(unmapped["reasoning"]["effort"], "xhigh");

    // Responses does not accept Chat's top-level reasoning_effort alias.
    let nonstandard = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning_effort": "xhigh"
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, nonstandard.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));

    // Verify that planning also preserves the canonical Chat level for every candidate.
    let chat = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "reasoning_effort": "xhigh"
    }))
    .unwrap();
    let prepared = support::prepare(&registry, ApiProtocol::ChatCompletions, chat.into()).unwrap();
    let mapped: Value = serde_json::from_slice(prepared.candidates()[0].request().body()).unwrap();
    let unmapped: Value =
        serde_json::from_slice(prepared.candidates()[1].request().body()).unwrap();
    assert_eq!(mapped["reasoning_effort"], "xhigh");
    assert_eq!(unmapped["reasoning_effort"], "xhigh");

    // Provider-private target values must not expand the public level set available downstream.
    let unsupported = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "max"}
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, unsupported.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));
}

#[test]
fn native_routes_accept_declared_none_and_max_reasoning_levels() {
    // A new canonical level must preserve stable wire round trips.
    assert_eq!(
        ReasoningLevel::from_wire("none"),
        Some(ReasoningLevel::None)
    );
    assert_eq!(ReasoningLevel::None.as_wire(), "none");
    assert_eq!(ReasoningLevel::from_wire("max"), Some(ReasoningLevel::Max));
    assert_eq!(ReasoningLevel::Max.as_wire(), "max");

    // After explicit model declaration, Chat and Responses requests preserve their respective levels.
    let mut definition = base_definition();
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::None, ReasoningLevel::Max];
    let registry = build_test_registry(definition);
    for (protocol, request, pointer, expected) in [
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "reasoning_effort": "max"
            }),
            "/reasoning_effort",
            "max",
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "hello",
                "reasoning": {"effort": "none"}
            }),
            "/reasoning/effort",
            "none",
        ),
    ] {
        let prepared = support::prepare(
            &registry,
            protocol,
            serde_json::to_vec(&request).unwrap().into(),
        )
        .unwrap();
        let upstream: Value = serde_json::from_slice(prepared.request().body()).unwrap();
        assert_eq!(
            upstream.pointer(pointer).and_then(Value::as_str),
            Some(expected)
        );
    }
}

#[test]
fn request_preflight_rejects_conflicting_reasoning_configuration_sources() {
    let mut definition = base_definition();
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::Low, ReasoningLevel::High];
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "reasoning": {"effort": "low"},
        "reasoning_effort": "high"
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::InvalidReasoningConfiguration
    ));
}

#[test]
fn bridged_reasoning_requires_a_readable_upstream_output_capability() {
    fn definition(reasoning_output: ReasoningOutput) -> RegistryConfig {
        let mut definition = base_definition();
        definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
        definition.models[0].reasoning = ReasoningSupport::Supported;
        definition.models[0].reasoning_levels = vec![ReasoningLevel::High];

        definition.credential_pools[0].id = "deepseek-primary".to_owned();
        definition.credential_pools[0].provider = openbridge::provider::ProviderKind::DeepSeek;
        let target = &mut definition.upstream_targets[0];
        target.provider = openbridge::provider::ProviderKind::DeepSeek;
        target.base_url = "https://api.deepseek.com".to_owned();
        target.credential_pool = "deepseek-primary".to_owned();
        target.upstream_apis.truncate(1);
        target.upstream_apis[0].endpoint_profile = "deepseek-openai".to_owned();
        if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
            &mut target.upstream_apis[0].capabilities
        {
            capabilities.reasoning_output = reasoning_output;
        }
        definition.routes = vec![RouteConfig {
            id: "responses-via-chat".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_api: "chat".to_owned(),
            downstream_operation: ApiProtocol::Responses.operation(),
            mode: RouteMode::Bridged,
        }];
        definition.public_models[0].routes = vec!["responses-via-chat".to_owned()];
        definition
    }

    let request = serde_json::to_vec(&serde_json::json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "high"}
    }))
    .unwrap();
    let unknown = build_test_registry(definition(ReasoningOutput::Unknown));
    assert!(matches!(
        support::prepare(&unknown, ApiProtocol::Responses, request.clone().into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));

    let readable = build_test_registry(definition(ReasoningOutput::PlainText));
    let plan = support::prepare(&readable, ApiProtocol::Responses, request.into())
        .expect("plain-text reasoning should remain bridgeable");
    assert!(plan.candidates()[0].bridge().is_some());
}

fn build_test_registry(definition: RegistryConfig) -> RuntimeRegistry {
    build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap()
}

#[test]
fn native_routing_preserves_original_request_for_the_provider_adapter() {
    let registry = build_test_registry(base_definition());
    let original = json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "hello"}],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "stream": true
    });

    let prepared = support::prepare(
        &registry,
        ApiProtocol::ChatCompletions,
        serde_json::to_vec(&original).unwrap().into(),
    )
    .unwrap();
    let preserved: Value = serde_json::from_slice(prepared.request().body()).unwrap();

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(preserved, original);
}

#[test]
fn public_model_preflight_rejects_unknown_models() {
    let registry = build_test_registry(base_definition());
    let body = serde_json::to_vec(&json!({"model": "missing", "messages": []})).unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnknownModel
    ));
}

#[test]
fn public_model_preflight_rejects_capabilities_not_guaranteed_by_its_contract() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn public_model_capability_rejection_does_not_select_a_stronger_later_route() {
    // Build a preferred Chat Route with weaker capability and a later Route supporting tools.
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let mut stronger = definition.upstream_targets[0].clone();
    stronger.id = "openai-stronger".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut stronger.upstream_apis[0].capabilities
    {
        capabilities.function_calling = true;
    }
    definition.upstream_targets.push(stronger);
    definition.routes.push(RouteConfig {
        id: "stronger-chat".to_owned(),
        upstream_target: "openai-stronger".to_owned(),
        upstream_api: "chat".to_owned(),
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes = vec!["public-chat".to_owned(), "stronger-chat".to_owned()];
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    // The Public Model fixed intersection does not support tools; a stronger later Route cannot change eligibility.
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn public_model_preflight_gates_output_parallel_image_and_reasoning_requirements() {
    let mut definition = base_definition();
    definition.models[0].context_length = ModelContextLength::new(None, None, Some(32));
    let registry = build_test_registry(definition);

    let too_large = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 33
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, too_large.into()).unwrap_err(),
        RequestPlanningError::OutputLimitExceeded
    ));

    let parallel = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "parallel_tool_calls": true
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, parallel.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let image = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{
            "role": "user",
            "content": [{"type": "image_url", "image_url": {"url": "https://example.invalid/image.png"}}]
        }]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, image.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "low"}
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, reasoning.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));

    let invalid_shape = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": false
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, invalid_shape.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));
}

#[test]
fn public_model_output_limit_uses_the_most_restrictive_route() {
    let mut definition = base_definition();
    let mut limited = definition.upstream_targets[0].clone();
    limited.id = "openai-limited".to_owned();
    limited.upstream_apis[0].upstream_model = "limited-upstream-model".to_owned();
    limited.upstream_apis[0].model_rules.context_length =
        ModelContextLength::new(None, None, Some(4_096));
    definition.upstream_targets.push(limited);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "limited-chat".to_owned(),
        upstream_target: "openai-limited".to_owned(),
        upstream_api: "chat".to_owned(),
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0].routes = vec!["limited-chat".to_owned(), "public-chat".to_owned()];
    let registry = build_test_registry(definition);
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 4097
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, request.into()).unwrap_err(),
        RequestPlanningError::OutputLimitExceeded
    ));
}

#[test]
fn public_model_interfaces_scope_capabilities_and_detect_strict_functions() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.store = true;
    }
    let registry = build_test_registry(definition);

    let chat_store = serde_json::to_vec(&json!({
        "model": "public-model", "messages": [], "store": true
    }))
    .unwrap();
    assert!(support::prepare(&registry, ApiProtocol::ChatCompletions, chat_store.into()).is_ok());

    let responses_store = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "store": true
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, responses_store.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let unmodeled_tool = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "tools": [{"type": "future_tool"}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, unmodeled_tool.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let strict_function = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe", "strict": true}}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(
            &registry,
            ApiProtocol::ChatCompletions,
            strict_function.into()
        )
        .unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.enabled = false;
    }
    let registry = build_test_registry(definition);
    let request = serde_json::to_vec(&json!({"model": "public-model", "input": "hello"})).unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap_err(),
        RequestPlanningError::UnsupportedProtocol
    ));
}

#[test]
fn route_plan_preserves_configured_order_after_public_model_preflight() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let mut tools = definition.upstream_targets[0].clone();
    tools.id = "openai-tools".to_owned();
    tools.upstream_apis[0].upstream_model = "tool-capable-model".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut tools.upstream_apis[0].capabilities
    {
        capabilities.function_calling = true;
    }
    definition.upstream_targets.push(tools);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "tools-chat".to_owned(),
        upstream_target: "openai-tools".to_owned(),
        upstream_api: "chat".to_owned(),
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("tools-chat".to_owned());
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": []
    }))
    .unwrap();

    let prepared = support::prepare(&registry, ApiProtocol::ChatCompletions, body.clone().into())
        .expect("a request accepted by the fixed contract should preserve configured routes");

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(prepared.candidates().len(), 2);
    assert_eq!(
        prepared.candidates()[1].upstream_target_id(),
        "openai-tools"
    );
    assert_eq!(prepared.request().body(), &body.as_slice());
}

#[test]
fn static_disabled_routes_do_not_contribute_to_the_compiled_interface_or_plan() {
    let mut definition = base_definition();

    // Keep a weaker configured Route disabled so it cannot narrow the visible contract or enter planning.
    definition.upstream_targets[0].enabled = false;
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let mut enabled = definition.upstream_targets[0].clone();
    enabled.id = "openai-enabled".to_owned();
    enabled.enabled = true;
    enabled.upstream_apis[0].upstream_model = "enabled-tool-model".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut enabled.upstream_apis[0].capabilities
    {
        capabilities.function_calling = true;
    }
    definition.upstream_targets.push(enabled);
    definition.routes.push(RouteConfig {
        id: "enabled-chat".to_owned(),
        upstream_target: "openai-enabled".to_owned(),
        upstream_api: "chat".to_owned(),
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes = vec![
        "public-chat".to_owned(),
        "enabled-chat".to_owned(),
        "public-responses".to_owned(),
    ];
    let registry = build_test_registry(definition);

    // Verify that only the enabled Route shapes both the published Chat contract and its candidates.
    let info = serde_json::to_value(
        registry
            .public_model("public-model")
            .expect("the enabled Chat interface must keep the model visible")
            .info(),
    )
    .unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["tools"]["support"],
        "supported"
    );
    let body = serde_json::to_vec(&json!({"model": "public-model", "messages": []})).unwrap();
    let plan = support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap();
    assert_eq!(
        plan.candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        ["enabled-chat"]
    );

    // Confirm that the same static contract admits the enabled Route's function-tool request.
    let tool_body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();
    let tool_plan = support::prepare(&registry, ApiProtocol::ChatCompletions, tool_body.into())
        .expect("the disabled weaker Route must not reject the enabled interface capability");
    assert_eq!(tool_plan.candidates()[0].route_id(), "enabled-chat");
}
