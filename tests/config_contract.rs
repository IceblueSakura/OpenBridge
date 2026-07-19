use openbridge::config::{ConfigError, ConfigManager, load_registry};
use openbridge::provider::ProviderKind;

const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
allowed_origins = ["https://api.openai.com"]
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
"#;

const ROUTES: &str = r#"
schema_version = 1
config_version = "test-1"

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
upstream_model = "test-model"
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
name = "code-primary"
candidates = ["openai-main"]
"#;

#[test]
fn valid_config_builds_a_resolved_registry() {
    let registry = load_registry(BOOTSTRAP, ROUTES).expect("valid config should load");

    assert_eq!(registry.version().as_str(), "test-1");
    assert_eq!(
        registry.provider("openai").unwrap().kind(),
        ProviderKind::OpenAi
    );
    assert_eq!(
        registry
            .deployment("openai-main")
            .unwrap()
            .origin()
            .as_str(),
        "https://api.openai.com/"
    );
    assert_eq!(
        registry.alias("code-primary").unwrap().candidates(),
        &["openai-main"]
    );
}

#[test]
fn reload_is_atomic_and_does_not_mutate_in_flight_snapshots() {
    let initial = load_registry(BOOTSTRAP, ROUTES).unwrap();
    let manager = ConfigManager::new(initial);
    let in_flight = manager.snapshot();

    let invalid = ROUTES.replace("kind = \"openai\"", "kind = \"unknown\"");
    assert!(manager.reload(BOOTSTRAP, &invalid).is_err());
    assert_eq!(manager.snapshot().version().as_str(), "test-1");

    let updated = ROUTES.replace("test-1", "test-2");
    manager.reload(BOOTSTRAP, &updated).unwrap();

    assert_eq!(manager.snapshot().version().as_str(), "test-2");
    assert_eq!(in_flight.version().as_str(), "test-1");
}

#[test]
fn plaintext_credentials_are_rejected_before_snapshot_creation() {
    let routes = ROUTES.replace("env://OPENAI_API_KEY", "sk-plaintext-secret");

    let error = load_registry(BOOTSTRAP, &routes).unwrap_err();

    assert!(matches!(
        error,
        ConfigError::InvalidSecretReference { provider } if provider == "openai"
    ));
}

#[test]
fn bootstrap_rejects_non_loopback_listeners() {
    let bootstrap = BOOTSTRAP.replace("127.0.0.1", "0.0.0.0");

    let error = load_registry(&bootstrap, ROUTES).unwrap_err();

    assert!(matches!(error, ConfigError::NonLoopbackListen { .. }));
}

#[test]
fn unknown_provider_kinds_are_rejected_during_load() {
    let routes = ROUTES.replace("kind = \"openai\"", "kind = \"dynamic\"");

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::UnknownProviderKind { kind } if kind == "dynamic"
    ));
}

#[test]
fn deployment_origin_must_be_in_bootstrap_allowlist() {
    let routes = ROUTES.replace("https://api.openai.com", "https://attacker.invalid");

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::OriginNotAllowed { .. }
    ));
}

#[test]
fn deployment_cannot_elevate_adapter_capabilities() {
    let routes = ROUTES.replace("background = false", "background = true");

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::CapabilityElevation { .. }
    ));
}

#[test]
fn unknown_route_config_fields_are_rejected() {
    let routes = ROUTES.replace(
        "config_version = \"test-1\"",
        "config_version = \"test-1\"\narbitrary_headers = true",
    );

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::RouteParse(_)
    ));
}
