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
upstream_model = "upstream-model"
endpoint_profile = "public-api"
base_url = "https://api.openai.com"
request_timeout_ms = 120000
[deployments.capabilities]
chat = true
responses = true
streaming = true
function_tools = true
structured_output = false
previous_response_id = false
background = false
response_store = false

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
    let routes = ROUTES.replace("function_tools = true", "function_tools = false");
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
