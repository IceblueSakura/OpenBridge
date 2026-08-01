mod support;

use openbridge::{
    core::Protocol,
    pipeline::RouteError,
    registry::{
        ModelContextLength, ReasoningLevel, ReasoningSupport, RegistryDefinition, RegistrySnapshot,
        build_registry,
    },
};
use serde_json::{Value, json};

fn base_definition() -> RegistryDefinition {
    support::definition("routing-test", "public-model", "upstream-model")
}

fn build_snapshot(definition: RegistryDefinition) -> RegistrySnapshot {
    build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap()
}

#[test]
fn native_routing_preserves_original_request_for_the_provider_adapter() {
    let snapshot = build_snapshot(base_definition());
    let original = json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "hello"}],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "stream": true
    });

    let prepared = support::prepare(
        &snapshot,
        Protocol::ChatCompletions,
        serde_json::to_vec(&original).unwrap().into(),
    )
    .unwrap();
    let preserved: Value = serde_json::from_slice(prepared.request().body()).unwrap();

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(preserved, original);
}

#[test]
fn native_routing_rejects_unknown_public_models() {
    let snapshot = build_snapshot(base_definition());
    let body = serde_json::to_vec(&json!({"model": "missing", "messages": []})).unwrap();

    assert!(matches!(
        support::prepare(&snapshot, Protocol::ChatCompletions, body.into()).unwrap_err(),
        RouteError::UnknownModel
    ));
}

#[test]
fn native_routing_rejects_features_disabled_by_the_offering() {
    let mut definition = base_definition();
    if let openbridge::registry::NativeOfferingCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].offerings[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let snapshot = build_snapshot(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&snapshot, Protocol::ChatCompletions, body.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));
}

#[test]
fn routing_gates_output_parallel_image_and_reasoning_requirements() {
    let mut definition = base_definition();
    definition.real_models[0].context_length = ModelContextLength::new(None, Some(32));
    let snapshot = build_snapshot(definition);

    let too_large = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 33
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::ChatCompletions, too_large.into()).unwrap_err(),
        RouteError::OutputLimitExceeded
    ));

    let parallel = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "parallel_tool_calls": true
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::ChatCompletions, parallel.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
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
        support::prepare(&snapshot, Protocol::ChatCompletions, image.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "low"}
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::Responses, reasoning.into()).unwrap_err(),
        RouteError::ReasoningLevelUnsupported
    ));

    let mut definition = base_definition();
    definition.real_models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.real_models[0].reasoning = ReasoningSupport::Supported;
    definition.real_models[0].reasoning_levels = vec![ReasoningLevel::Low];
    let snapshot = build_snapshot(definition);
    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning_effort": "low"
    }))
    .unwrap();
    assert!(support::prepare(&snapshot, Protocol::Responses, reasoning.into()).is_ok());
}

#[test]
fn offering_constraints_select_the_unconstrained_candidate() {
    let mut definition = base_definition();
    let mut limited = definition.upstream_targets[0].clone();
    limited.id = "openai-limited".to_owned();
    limited.offerings[0].upstream_model = "limited-upstream-model".to_owned();
    limited.offerings[0].model_constraints.context_length =
        ModelContextLength::new(None, Some(4_096));
    definition.upstream_targets.push(limited);
    definition
        .serving_routes
        .push(openbridge::registry::ServingRouteDefinition {
            id: "limited-chat".to_owned(),
            upstream_target: "openai-limited".to_owned(),
            offering: "chat".to_owned(),
            downstream_protocol: Protocol::ChatCompletions,
            mode: openbridge::registry::ServingRouteMode::Native,
        });
    definition.public_models[0].serving_routes =
        vec!["limited-chat".to_owned(), "public-chat".to_owned()];
    let snapshot = build_snapshot(definition);
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 4097
    }))
    .unwrap();

    let prepared = support::prepare(&snapshot, Protocol::ChatCompletions, request.into())
        .expect("the unconstrained candidate should remain eligible");

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(prepared.candidates().len(), 1);
}

#[test]
fn routing_scopes_capabilities_by_protocol_and_detects_strict_functions() {
    let mut definition = base_definition();
    if let openbridge::registry::NativeOfferingCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].offerings[0].capabilities
    {
        capabilities.store = true;
    }
    let snapshot = build_snapshot(definition);

    let chat_store = serde_json::to_vec(&json!({
        "model": "public-model", "messages": [], "store": true
    }))
    .unwrap();
    assert!(support::prepare(&snapshot, Protocol::ChatCompletions, chat_store.into()).is_ok());

    let responses_store = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "store": true
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::Responses, responses_store.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let unmodeled_tool = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "tools": [{"type": "web_search"}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::Responses, unmodeled_tool.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let strict_function = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe", "strict": true}}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::ChatCompletions, strict_function.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let mut definition = base_definition();
    if let openbridge::registry::NativeOfferingCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].offerings[1].capabilities
    {
        capabilities.enabled = false;
    }
    let snapshot = build_snapshot(definition);
    let request = serde_json::to_vec(&json!({"model": "public-model", "input": "hello"})).unwrap();
    assert!(matches!(
        support::prepare(&snapshot, Protocol::Responses, request.into()).unwrap_err(),
        RouteError::UnsupportedProtocol
    ));
}

#[test]
fn native_routing_selects_the_first_capability_compatible_candidate() {
    let mut definition = base_definition();
    if let openbridge::registry::NativeOfferingCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].offerings[0].capabilities
    {
        capabilities.function_calling = false;
    }
    let mut tools = definition.upstream_targets[0].clone();
    tools.id = "openai-tools".to_owned();
    tools.offerings[0].upstream_model = "tool-capable-model".to_owned();
    if let openbridge::registry::NativeOfferingCapabilities::ChatCompletions(capabilities) =
        &mut tools.offerings[0].capabilities
    {
        capabilities.function_calling = true;
    }
    definition.upstream_targets.push(tools);
    definition
        .serving_routes
        .push(openbridge::registry::ServingRouteDefinition {
            id: "tools-chat".to_owned(),
            upstream_target: "openai-tools".to_owned(),
            offering: "chat".to_owned(),
            downstream_protocol: Protocol::ChatCompletions,
            mode: openbridge::registry::ServingRouteMode::Native,
        });
    definition.public_models[0]
        .serving_routes
        .push("tools-chat".to_owned());
    let snapshot = build_snapshot(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    let prepared = support::prepare(&snapshot, Protocol::ChatCompletions, body.clone().into())
        .expect("a later compatible candidate should be selected");

    assert_eq!(prepared.upstream_target_id(), "openai-tools");
    assert_eq!(prepared.request().body(), &body.as_slice());
}
