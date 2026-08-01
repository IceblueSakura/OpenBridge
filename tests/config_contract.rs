mod support;

use std::{path::PathBuf, time::Duration};

use openbridge::{
    config::{
        BootstrapConfigError, BootstrapConfigFileError, BootstrapConfigPath, parse_bootstrap_config,
    },
    registry::{
        ModelContextLength, PublicModelConfig, ReasoningLevel, ReasoningSupport, RegistryError,
        UpstreamApiCapabilities, build_registry,
    },
};

use support::{BOOTSTRAP, bootstrap, definition};

#[test]
fn bootstrap_and_code_registry_build_a_runtime_registry() {
    let mut definition = definition("test-1", "code-primary", "test-model");
    definition.models[0].supported_parameters = vec![
        "max_tokens".to_owned(),
        "tools".to_owned(),
        "reasoning".to_owned(),
    ];
    definition.models[0].reasoning = ReasoningSupport::Supported;

    let registry = build_registry(bootstrap(BOOTSTRAP), definition).unwrap();

    assert_eq!(registry.version().as_str(), "test-1");
    assert_eq!(registry.listen().to_string(), "127.0.0.1:8080");
    assert_eq!(registry.limits().max_request_body_bytes(), 1_048_576);
    assert_eq!(
        registry.http_client().connect_timeout(),
        Duration::from_secs(5)
    );
    let target = registry.upstream_target("openai-main").unwrap();
    assert_eq!(target.credential().secret_reference().scheme(), "env");
    assert_eq!(
        target.credential().secret_reference().locator(),
        "OPENAI_API_KEY"
    );
    let upstream_api = target.upstream_api("chat").unwrap();
    assert_eq!(upstream_api.upstream_model(), "test-model");
    assert_eq!(target.endpoint_base().as_str(), "https://api.openai.com/");
    assert_eq!(
        registry.public_model("code-primary").unwrap().routes(),
        &["public-chat", "public-responses"]
    );
}

#[test]
fn bootstrap_path_only_loads_process_policy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = BootstrapConfigPath::new(root.join("config/bootstrap.toml"));
    let policy = path.load().unwrap();

    assert!(policy.listen().ip().is_loopback());
    assert!(path.path().ends_with("config/bootstrap.toml"));

    let missing = BootstrapConfigPath::new(root.join("config/missing-bootstrap.toml"));
    assert!(matches!(
        missing.load().unwrap_err(),
        BootstrapConfigFileError::Read { .. }
    ));
}

#[test]
fn bootstrap_rejects_unknown_fields_non_loopback_and_zero_limits() {
    let unknown = BOOTSTRAP.replace(
        "listen = \"127.0.0.1:8080\"",
        "listen = \"127.0.0.1:8080\"\nprovider = \"dynamic\"",
    );
    assert!(matches!(
        parse_bootstrap_config(&unknown),
        Err(BootstrapConfigError::Parse)
    ));

    let non_loopback = BOOTSTRAP.replace("127.0.0.1", "0.0.0.0");
    assert!(matches!(
        parse_bootstrap_config(&non_loopback),
        Err(BootstrapConfigError::NonLoopbackListen { .. })
    ));

    let zero = BOOTSTRAP.replace("max_sse_event_bytes = 262144", "max_sse_event_bytes = 0");
    assert!(matches!(
        parse_bootstrap_config(&zero),
        Err(BootstrapConfigError::InvalidLimit {
            name: "max_sse_event_bytes"
        })
    ));
}

#[test]
fn model_config_and_typed_rules_are_validated() {
    let mut invalid = definition("test", "code-primary", "test-model");
    invalid.models[0].context_length = ModelContextLength::new(None, Some(0));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid),
        Err(RegistryError::InvalidModelContextLength { .. })
    ));

    let mut duplicate = definition("test", "code-primary", "test-model");
    duplicate.models[0].supported_parameters = vec!["tools".to_owned(), "tools".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::DuplicateSupportedParameter { .. })
    ));

    let mut inconsistent = definition("test", "code-primary", "test-model");
    inconsistent.models[0].reasoning = ReasoningSupport::Supported;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), inconsistent),
        Err(RegistryError::InconsistentReasoningConfig { .. })
    ));

    let mut invalid_levels = definition("test", "code-primary", "test-model");
    invalid_levels.models[0].reasoning_levels = vec![ReasoningLevel::Low];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_levels),
        Err(RegistryError::InconsistentReasoningConfig { .. })
    ));

    let mut duplicate_levels = definition("test", "code-primary", "test-model");
    duplicate_levels.models[0].supported_parameters = vec!["reasoning".to_owned()];
    duplicate_levels.models[0].reasoning = ReasoningSupport::Supported;
    duplicate_levels.models[0].reasoning_levels = vec![ReasoningLevel::High, ReasoningLevel::High];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate_levels),
        Err(RegistryError::InconsistentReasoningConfig { .. })
    ));
}

#[test]
fn upstream_api_rules_only_reduce_model_info() {
    let mut definition = definition("test", "code-primary", "test-model");
    definition.models[0].supported_parameters =
        vec!["max_tokens".to_owned(), "reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .context_length = ModelContextLength::new(Some(64_000), Some(4_096));
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .reasoning = Some(ReasoningSupport::Unsupported);
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .disabled_parameters = vec!["reasoning".to_owned()];

    let registry = build_registry(bootstrap(BOOTSTRAP), definition).unwrap();
    let base = registry.model("openai/test-model").unwrap();
    let effective = registry
        .upstream_target("openai-main")
        .unwrap()
        .upstream_api("chat")
        .unwrap()
        .model();

    assert_eq!(base.context_length().output_tokens(), Some(8_192));
    assert_eq!(base.reasoning(), ReasoningSupport::Supported);
    assert_eq!(effective.context_length().output_tokens(), Some(4_096));
    assert_eq!(effective.reasoning(), ReasoningSupport::Unsupported);
    assert_eq!(effective.supported_parameters(), ["max_tokens"]);
}

#[test]
fn upstream_api_rules_cannot_widen_model_info() {
    let mut widened = definition("test", "code-primary", "test-model");
    widened.upstream_targets[0].upstream_apis[0]
        .model_rules
        .context_length = ModelContextLength::new(None, Some(8_193));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), widened),
        Err(RegistryError::UpstreamApiModelLimitExceedsModel { .. })
    ));

    let mut definition = definition("test", "code-primary", "test-model");
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .reasoning = Some(ReasoningSupport::Supported);
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), definition),
        Err(RegistryError::UpstreamApiModelRuleWidensModel { .. })
    ));
}

#[test]
fn endpoint_registration_accepts_safe_prefixes_and_rejects_unsafe_urls() {
    for base_url in [
        "https://api.openai.com/openai",
        "https://api.openai.com/openai/",
    ] {
        let mut definition = definition("test", "code-primary", "test-model");
        definition.upstream_targets[0].base_url = base_url.to_owned();
        let registry = build_registry(bootstrap(BOOTSTRAP), definition).unwrap();
        assert_eq!(
            registry
                .upstream_target("openai-main")
                .unwrap()
                .endpoint_base()
                .as_str(),
            "https://api.openai.com/openai/"
        );
    }

    for base_url in [
        "http://api.openai.com",
        "https://api.openai.com/openai?target=attacker.invalid",
        "https://api.openai.com/openai//admin",
        "https://api.openai.com/openai/%2Fadmin",
    ] {
        let mut definition = definition("test", "code-primary", "test-model");
        definition.upstream_targets[0].base_url = base_url.to_owned();
        assert!(matches!(
            build_registry(bootstrap(BOOTSTRAP), definition),
            Err(RegistryError::InvalidBaseUrl { .. })
        ));
    }
}

#[test]
fn registry_rejects_duplicate_and_unknown_references() {
    let mut duplicate = definition("test", "code-primary", "test-model");
    duplicate.models.push(duplicate.models[0].clone());
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::DuplicateId {
            entity: "model",
            ..
        })
    ));

    let mut unknown = definition("test", "code-primary", "test-model");
    unknown.public_models[0].routes = vec!["missing".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), unknown),
        Err(RegistryError::UnknownReference {
            entity: "public model",
            ..
        })
    ));

    let mut duplicate_candidate = definition("test", "code-primary", "test-model");
    duplicate_candidate.public_models = vec![PublicModelConfig {
        name: "code-primary".to_owned(),
        routes: vec!["public-chat".to_owned(), "public-chat".to_owned()],
    }];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate_candidate),
        Err(RegistryError::DuplicatePublicModelRoute { .. })
    ));
}

#[test]
fn registry_rejects_capability_elevation_and_invalid_credential_locator() {
    let mut elevation = definition("test", "code-primary", "test-model");
    if let UpstreamApiCapabilities::Responses(capabilities) =
        &mut elevation.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.background = true;
    }
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), elevation),
        Err(RegistryError::CapabilityElevation { .. })
    ));

    let mut locator = definition("test", "code-primary", "test-model");
    locator.upstream_targets[0].credential.environment_variable = "not-valid".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), locator),
        Err(RegistryError::InvalidCredentialLocator { .. })
    ));
}
