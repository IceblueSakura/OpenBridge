use openbridge::{
    config::load_registry,
    core::Protocol,
    pipeline::{RouteError, prepare_native_request},
};
use serde_json::{Value, json};

const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
allowed_origins = ["https://api.openai.com"]
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

const ROUTES: &str = r#"
schema_version = 1
config_version = "routing-test"

[[models]]
id = "openai/upstream-model"
name = "Upstream model"
supported_parameters = ["reasoning"]
reasoning = "unknown"

[[providers]]
id = "openai"
kind = "openai"
[providers.credential]
id = "openai-primary"
kind = "api_key"
secret_ref = "env://OPENAI_API_KEY"

[[deployments]]
id = "openai-main"
provider = "openai"
model = "openai/upstream-model"
upstream_model = "upstream-model"
endpoint_profile = "public-api"
base_url = "https://api.openai.com"
request_timeout_ms = 120000
[deployments.capabilities.chat_completions]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false

[deployments.capabilities.responses]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false
previous_response_id = false
background = false

[[aliases]]
name = "public-model"
candidates = ["openai-main"]
"#;

#[test]
fn native_routing_rewrites_only_model_for_both_protocols() {
    let snapshot = load_registry(BOOTSTRAP, ROUTES).unwrap();
    let cases = [
        (
            Protocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "tools": [{"type": "function", "function": {"name": "probe"}}],
                "stream": true
            }),
        ),
        (
            Protocol::Responses,
            json!({
                "model": "public-model",
                "input": "hello",
                "text": {"format": {"type": "text"}},
                "tools": [{"type": "function", "name": "probe"}],
                "stream": true
            }),
        ),
    ];

    for (protocol, original) in cases {
        let prepared = prepare_native_request(
            &snapshot,
            protocol,
            serde_json::to_vec(&original).unwrap().into(),
        )
        .unwrap();
        let rewritten: Value = serde_json::from_slice(prepared.request().body()).unwrap();
        let mut expected = original;
        expected["model"] = json!("upstream-model");

        assert_eq!(prepared.deployment_id(), "openai-main");
        assert_eq!(rewritten, expected);
    }
}

#[test]
fn native_routing_rejects_unknown_public_models() {
    let snapshot = load_registry(BOOTSTRAP, ROUTES).unwrap();
    let body = serde_json::to_vec(&json!({"model": "missing", "messages": []})).unwrap();

    let error =
        prepare_native_request(&snapshot, Protocol::ChatCompletions, body.into()).unwrap_err();

    assert!(matches!(error, RouteError::UnknownModel));
}

#[test]
fn native_routing_rejects_features_disabled_by_the_deployment() {
    let routes = ROUTES.replacen("function_calling = true", "function_calling = false", 1);
    let snapshot = load_registry(BOOTSTRAP, &routes).unwrap();
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    let error =
        prepare_native_request(&snapshot, Protocol::ChatCompletions, body.into()).unwrap_err();

    assert!(matches!(error, RouteError::UnsupportedCapabilities));
}

#[test]
fn native_routing_selects_output_limit_compatible_candidates_and_gates_explicit_parallel_tools() {
    let routes = ROUTES.replace(
        "[[aliases]]",
        r#"[models.context_length]
output = 32

[[aliases]]"#,
    );
    let snapshot = load_registry(BOOTSTRAP, &routes).unwrap();

    let too_large = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 33,
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, too_large.into()).unwrap_err(),
        RouteError::OutputLimitExceeded
    ));

    let permitted = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "max_output_tokens": 32,
    }))
    .unwrap();
    assert!(prepare_native_request(&snapshot, Protocol::Responses, permitted.into()).is_ok());

    let parallel = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "parallel_tool_calls": true,
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
            "content": [{"type": "image_url", "image_url": {"url": "https://example.invalid/image.png"}}],
        }],
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, image.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "low"},
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, reasoning.into()).unwrap_err(),
        RouteError::ReasoningUnsupported
    ));

    let reasoning_supported =
        ROUTES.replace("reasoning = \"unknown\"", "reasoning = \"supported\"");
    let snapshot = load_registry(BOOTSTRAP, &reasoning_supported).unwrap();
    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning_effort": "low",
    }))
    .unwrap();
    assert!(prepare_native_request(&snapshot, Protocol::Responses, reasoning.into()).is_ok());
}

#[test]
fn native_routing_scopes_capabilities_by_protocol_and_detects_strict_function_calling() {
    let chat_store_only = ROUTES.replacen("store = false", "store = true", 1);
    let snapshot = load_registry(BOOTSTRAP, &chat_store_only).unwrap();

    let chat_store = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "store": true,
    }))
    .unwrap();
    assert!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, chat_store.into()).is_ok()
    );

    let responses_store = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "store": true,
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, responses_store.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let unmodeled_tool = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "tools": [{"type": "web_search"}]
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, unmodeled_tool.into()).unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let strict_function = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{
            "type": "function",
            "function": {"name": "probe", "strict": true}
        }]
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::ChatCompletions, strict_function.into())
            .unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let strict_response_function = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "tools": [{"type": "function", "name": "probe", "strict": true}]
    }))
    .unwrap();
    assert!(matches!(
        prepare_native_request(
            &snapshot,
            Protocol::Responses,
            strict_response_function.into()
        )
        .unwrap_err(),
        RouteError::UnsupportedCapabilities
    ));

    let responses_disabled = ROUTES.replace(
        "[deployments.capabilities.responses]\nenabled = true",
        "[deployments.capabilities.responses]\nenabled = false",
    );
    let snapshot = load_registry(BOOTSTRAP, &responses_disabled).unwrap();
    let request = serde_json::to_vec(&json!({"model": "public-model", "input": "hello"})).unwrap();
    assert!(matches!(
        prepare_native_request(&snapshot, Protocol::Responses, request.into()).unwrap_err(),
        RouteError::UnsupportedProtocol
    ));
}

#[test]
fn native_routing_selects_the_first_capability_compatible_candidate() {
    let routes = ROUTES
        .replacen("function_calling = true", "function_calling = false", 1)
        .replace(
            "[[aliases]]",
            r#"[[deployments]]
id = "openai-tools"
provider = "openai"
model = "openai/upstream-model"
upstream_model = "tool-capable-model"
endpoint_profile = "public-api"
base_url = "https://api.openai.com"
request_timeout_ms = 120000
[deployments.capabilities.chat_completions]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false

[deployments.capabilities.responses]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false
previous_response_id = false
background = false

[[aliases]]"#,
        )
        .replace(
            "candidates = [\"openai-main\"]",
            "candidates = [\"openai-main\", \"openai-tools\"]",
        );
    let snapshot = load_registry(BOOTSTRAP, &routes).unwrap();
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    let prepared = prepare_native_request(&snapshot, Protocol::ChatCompletions, body.into())
        .expect("a later compatible candidate should be selected");
    let rewritten: Value = serde_json::from_slice(prepared.request().body()).unwrap();

    assert_eq!(prepared.deployment_id(), "openai-tools");
    assert_eq!(rewritten["model"], "tool-capable-model");
}
