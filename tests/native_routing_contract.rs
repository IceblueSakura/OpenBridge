mod support;

use openbridge::{
    core::Protocol,
    pipeline::{RouteError, prepare_native_request},
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

    let prepared = prepare_native_request(
        &snapshot,
        Protocol::ChatCompletions,
        serde_json::to_vec(&original).unwrap().into(),
    )
    .unwrap();
    let preserved: Value = serde_json::from_slice(prepared.request().body()).unwrap();

    assert_eq!(prepared.deployment_id(), "openai-main");
    assert_eq!(preserved, original);
}

#[test]
fn native_routing_rejects_unknown_public_models() {
    let snapshot = build_snapshot(base_definition());
    let body = serde_json::to_vec(&json!({"model": "missing", "messages": []})).unwrap();

    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, body.into()).unwrap_err(),
        RouteError::UnknownModel
    ));
}

#[test]
fn native_routing_rejects_features_disabled_by_the_deployment() {
    let mut definition = base_definition();
    definition.deployments[0]
        .capabilities
        .chat_completions
        .function_calling = false;
    let snapshot = build_snapshot(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, body.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));
}

#[test]
fn routing_gates_output_parallel_image_and_reasoning_requirements() {
    let mut definition = base_definition();
    definition.models[0].context_length = ModelContextLength::new(None, Some(32));
    let snapshot = build_snapshot(definition);

    let too_large = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 33
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, too_large.into()).unwrap_err(),
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
        prepare_native_request(&snapshot, Protocol::ChatCompletions, parallel.into()).unwrap_err(),
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
        prepare_native_request(&snapshot, Protocol::ChatCompletions, image.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "low"}
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, reasoning.into()).unwrap_err(),
        RouteError::ReasoningLevelUnsupported
    ));

    let mut definition = base_definition();
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::Low];
    let snapshot = build_snapshot(definition);
    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning_effort": "low"
    }))
    .unwrap();
    assert!(prepare_native_request(&snapshot, Protocol::Responses, reasoning.into()).is_ok());
}

#[test]
fn deployment_constraints_select_the_unconstrained_candidate() {
    let mut definition = base_definition();
    let mut limited = definition.deployments[0].clone();
    limited.id = "openai-limited".to_owned();
    limited.upstream_model = "limited-upstream-model".to_owned();
    limited.model_constraints.context_length = ModelContextLength::new(None, Some(4_096));
    definition.deployments.push(limited);
    definition.aliases[0].candidates = vec!["openai-limited".to_owned(), "openai-main".to_owned()];
    let snapshot = build_snapshot(definition);
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 4097
    }))
    .unwrap();

    let prepared = prepare_native_request(&snapshot, Protocol::ChatCompletions, request.into())
        .expect("the unconstrained candidate should remain eligible");

    assert_eq!(prepared.deployment_id(), "openai-main");
    assert_eq!(prepared.candidates().len(), 1);
}

#[test]
fn routing_scopes_capabilities_by_protocol_and_detects_strict_functions() {
    let mut definition = base_definition();
    definition.deployments[0]
        .capabilities
        .chat_completions
        .store = true;
    let snapshot = build_snapshot(definition);

    let chat_store = serde_json::to_vec(&json!({
        "model": "public-model", "messages": [], "store": true
    }))
    .unwrap();
    assert!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, chat_store.into()).is_ok()
    );

    let responses_store = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "store": true
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, responses_store.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let unmodeled_tool = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "tools": [{"type": "web_search"}]
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, unmodeled_tool.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let strict_function = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe", "strict": true}}]
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, strict_function.into())
            .unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let mut definition = base_definition();
    definition.deployments[0].capabilities.responses.enabled = false;
    let snapshot = build_snapshot(definition);
    let request = serde_json::to_vec(&json!({"model": "public-model", "input": "hello"})).unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, request.into()).unwrap_err(),
        RouteError::UnsupportedProtocol
    ));
}

#[test]
fn native_routing_selects_the_first_capability_compatible_candidate() {
    let mut definition = base_definition();
    definition.deployments[0]
        .capabilities
        .chat_completions
        .function_calling = false;
    let mut tools = definition.deployments[0].clone();
    tools.id = "openai-tools".to_owned();
    tools.upstream_model = "tool-capable-model".to_owned();
    tools.capabilities.chat_completions.function_calling = true;
    definition.deployments.push(tools);
    definition.aliases[0]
        .candidates
        .push("openai-tools".to_owned());
    let snapshot = build_snapshot(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    let prepared =
        prepare_native_request(&snapshot, Protocol::ChatCompletions, body.clone().into())
            .expect("a later compatible candidate should be selected");

    assert_eq!(prepared.deployment_id(), "openai-tools");
    assert_eq!(prepared.request().body(), &body.as_slice());
}
