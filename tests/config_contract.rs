use std::time::Duration;

use openbridge::config::{ConfigError, ConfigManager, load_registry};
use openbridge::provider::{CredentialKind, ProviderKind};

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
    let provider = registry.provider("openai").unwrap();
    assert_eq!(provider.kind(), ProviderKind::OpenAi);
    assert_eq!(provider.credential().id(), "openai-primary");
    assert_eq!(provider.credential().kind(), CredentialKind::ApiKey);
    assert_eq!(provider.credential().secret_reference().scheme(), "env");
    assert_eq!(
        provider.credential().secret_reference().locator(),
        "OPENAI_API_KEY"
    );
    assert_eq!(registry.limits().max_request_body_bytes(), 1_048_576);
    assert_eq!(registry.limits().max_sse_event_bytes(), 262_144);
    assert_eq!(
        registry.upstream_policy().connect_timeout(),
        Duration::from_secs(5)
    );
    assert_eq!(
        registry.upstream_policy().pool_idle_timeout(),
        Duration::from_secs(90)
    );
    assert_eq!(registry.upstream_policy().pool_max_idle_per_host(), 16);
    let deployment = registry.deployment("openai-main").unwrap();
    assert_eq!(deployment.provider_id(), "openai");
    assert_eq!(deployment.upstream_model(), "test-model");
    assert_eq!(deployment.endpoint_profile(), "public-api");
    assert_eq!(deployment.request_timeout(), Duration::from_secs(120));
    assert!(deployment.capabilities().responses);
    assert_eq!(deployment.origin().as_str(), "https://api.openai.com/");
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
fn reload_rejects_bootstrap_policy_changes() {
    let changed_bootstraps = [
        BOOTSTRAP.replace("127.0.0.1:8080", "127.0.0.1:8081"),
        BOOTSTRAP.replace(
            "[\"https://api.openai.com\"]",
            "[\"https://api.openai.com\", \"https://other.invalid\"]",
        ),
        BOOTSTRAP.replace(
            "max_request_body_bytes = 1048576",
            "max_request_body_bytes = 2048",
        ),
        BOOTSTRAP.replace(
            "upstream_pool_max_idle_per_host = 16",
            "upstream_pool_max_idle_per_host = 8",
        ),
    ];
    let updated_routes = ROUTES.replace("test-1", "test-2");

    for changed_bootstrap in changed_bootstraps {
        let initial = load_registry(BOOTSTRAP, ROUTES).unwrap();
        let manager = ConfigManager::new(initial);

        let error = manager
            .reload(&changed_bootstrap, &updated_routes)
            .unwrap_err();

        assert!(matches!(error, ConfigError::BootstrapPolicyChanged));
        assert_eq!(manager.snapshot().version().as_str(), "test-1");
        assert_eq!(
            manager.snapshot().limits().max_request_body_bytes(),
            1_048_576
        );
    }
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
        ConfigError::RouteParse
    ));
}

#[test]
fn route_parse_errors_do_not_echo_source_text() {
    let routes = ROUTES.replace(
        "secret_ref = \"env://OPENAI_API_KEY\"",
        "secret_ref = \"SENSITIVE_LOCATOR_MUST_NOT_APPEAR",
    );

    let error = load_registry(BOOTSTRAP, &routes).unwrap_err();
    let rendered = format!("{error:?} {error}");

    assert!(!rendered.contains("SENSITIVE_LOCATOR_MUST_NOT_APPEAR"));
}

#[test]
fn unsupported_credential_kinds_are_rejected() {
    let routes = ROUTES.replace("kind = \"api_key\"", "kind = \"oauth\"");

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::UnsupportedCredentialKind { provider, kind }
            if provider == "openai" && kind == "oauth"
    ));
}

#[test]
fn unavailable_secret_reference_backends_are_rejected() {
    let routes = ROUTES.replace("env://OPENAI_API_KEY", "vault://openai/key");

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::InvalidSecretReference { .. }
    ));
}

#[test]
fn zero_runtime_limits_are_rejected() {
    let bootstrap = BOOTSTRAP.replace("max_sse_event_bytes = 262144", "max_sse_event_bytes = 0");

    assert!(matches!(
        load_registry(&bootstrap, ROUTES).unwrap_err(),
        ConfigError::InvalidLimit {
            name: "max_sse_event_bytes"
        }
    ));
}

#[test]
fn zero_upstream_pool_policy_values_are_rejected() {
    let bootstrap = BOOTSTRAP.replace(
        "upstream_pool_idle_timeout_ms = 90000",
        "upstream_pool_idle_timeout_ms = 0",
    );

    assert!(matches!(
        load_registry(&bootstrap, ROUTES).unwrap_err(),
        ConfigError::InvalidLimit {
            name: "upstream_pool_idle_timeout_ms"
        }
    ));
}

#[test]
fn duplicate_alias_candidates_are_rejected() {
    let routes = ROUTES.replace(
        "candidates = [\"openai-main\"]",
        "candidates = [\"openai-main\", \"openai-main\"]",
    );

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::DuplicateAliasCandidate { alias, candidate }
            if alias == "code-primary" && candidate == "openai-main"
    ));
}

#[test]
fn zero_request_timeouts_are_rejected() {
    let routes = ROUTES.replace("request_timeout_ms = 120000", "request_timeout_ms = 0");

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::InvalidRequestTimeout { deployment } if deployment == "openai-main"
    ));
}

#[test]
fn credential_binding_ids_must_be_unique() {
    let duplicate_provider = r#"
[[providers]]
id = "openai-secondary"
kind = "openai"

[providers.credential]
id = "openai-primary"
kind = "api_key"
secret_ref = "env://OPENAI_SECONDARY_API_KEY"

[[deployments]]
"#;
    let routes = ROUTES.replace("[[deployments]]", duplicate_provider);

    assert!(matches!(
        load_registry(BOOTSTRAP, &routes).unwrap_err(),
        ConfigError::DuplicateId { entity: "credential", id } if id == "openai-primary"
    ));
}
