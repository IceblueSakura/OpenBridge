mod support;

use openbridge::{
    core::ApiProtocol,
    pipeline::RequestPlanningError,
    registry::{
        ModelContextLength, ReasoningLevel, ReasoningSupport, RegistryConfig, RuntimeRegistry,
        build_registry,
    },
};
use serde_json::{Value, json};

fn base_definition() -> RegistryConfig {
    support::definition("routing-test", "public-model", "upstream-model")
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
fn native_routing_rejects_unknown_public_models() {
    let registry = build_test_registry(base_definition());
    let body = serde_json::to_vec(&json!({"model": "missing", "messages": []})).unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnknownModel
    ));
}

#[test]
fn native_routing_rejects_features_disabled_by_the_upstream_api() {
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
fn routing_gates_output_parallel_image_and_reasoning_requirements() {
    let mut definition = base_definition();
    definition.models[0].context_length = ModelContextLength::new(None, Some(32));
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

    let mut definition = base_definition();
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::Low];
    let registry = build_test_registry(definition);
    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning_effort": "low"
    }))
    .unwrap();
    assert!(support::prepare(&registry, ApiProtocol::Responses, reasoning.into()).is_ok());
}

#[test]
fn upstream_api_rules_select_the_unconstrained_candidate() {
    let mut definition = base_definition();
    let mut limited = definition.upstream_targets[0].clone();
    limited.id = "openai-limited".to_owned();
    limited.credential.id = "openai-limited-credential".to_owned();
    limited.upstream_apis[0].upstream_model = "limited-upstream-model".to_owned();
    limited.upstream_apis[0].model_rules.context_length =
        ModelContextLength::new(None, Some(4_096));
    definition.upstream_targets.push(limited);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "limited-chat".to_owned(),
        upstream_target: "openai-limited".to_owned(),
        upstream_api: "chat".to_owned(),
        downstream_protocol: ApiProtocol::ChatCompletions,
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

    let prepared = support::prepare(&registry, ApiProtocol::ChatCompletions, request.into())
        .expect("the unconstrained candidate should remain eligible");

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(prepared.candidates().len(), 1);
}

#[test]
fn routing_scopes_capabilities_by_protocol_and_detects_strict_functions() {
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
        "model": "public-model", "input": "hello", "tools": [{"type": "web_search"}]
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
fn native_routing_selects_the_first_capability_compatible_candidate() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let mut tools = definition.upstream_targets[0].clone();
    tools.id = "openai-tools".to_owned();
    tools.credential.id = "openai-tools-credential".to_owned();
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
        downstream_protocol: ApiProtocol::ChatCompletions,
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("tools-chat".to_owned());
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    let prepared = support::prepare(&registry, ApiProtocol::ChatCompletions, body.clone().into())
        .expect("a later compatible candidate should be selected");

    assert_eq!(prepared.upstream_target_id(), "openai-tools");
    assert_eq!(prepared.request().body(), &body.as_slice());
}
