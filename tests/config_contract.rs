mod support;

use std::{path::PathBuf, time::Duration};

use openbridge::{
    config::{BootstrapError, BootstrapFileError, BootstrapPath, load_bootstrap},
    registry::{
        AliasDefinition, ModelContextLength, ReasoningLevel, ReasoningSupport, RegistryError,
        build_registry,
    },
};

use support::{BOOTSTRAP, bootstrap, definition};

#[test]
fn bootstrap_and_code_registry_build_a_resolved_snapshot() {
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
        registry.upstream_policy().connect_timeout(),
        Duration::from_secs(5)
    );
    let provider = registry.provider("openai").unwrap();
    assert_eq!(provider.credential().secret_reference().scheme(), "env");
    assert_eq!(
        provider.credential().secret_reference().locator(),
        "OPENAI_API_KEY"
    );
    let deployment = registry.deployment("openai-main").unwrap();
    assert_eq!(deployment.upstream_model(), "test-model");
    assert_eq!(
        deployment.endpoint_base().as_str(),
        "https://api.openai.com/"
    );
    assert_eq!(
        registry.alias("code-primary").unwrap().candidates(),
        &["openai-main"]
    );
}

#[test]
fn bootstrap_path_only_loads_process_policy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = BootstrapPath::new(root.join("config/bootstrap.toml"));
    let policy = path.load().unwrap();

    assert!(policy.listen().ip().is_loopback());
    assert!(path.path().ends_with("config/bootstrap.toml"));

    let missing = BootstrapPath::new(root.join("config/missing-bootstrap.toml"));
    assert!(matches!(
        missing.load().unwrap_err(),
        BootstrapFileError::Read { .. }
    ));
}

#[test]
fn bootstrap_rejects_unknown_fields_non_loopback_and_zero_limits() {
    let unknown = BOOTSTRAP.replace(
        "listen = \"127.0.0.1:8080\"",
        "listen = \"127.0.0.1:8080\"\nprovider = \"dynamic\"",
    );
    assert!(matches!(
        load_bootstrap(&unknown),
        Err(BootstrapError::Parse)
    ));

    let non_loopback = BOOTSTRAP.replace("127.0.0.1", "0.0.0.0");
    assert!(matches!(
        load_bootstrap(&non_loopback),
        Err(BootstrapError::NonLoopbackListen { .. })
    ));

    let zero = BOOTSTRAP.replace("max_sse_event_bytes = 262144", "max_sse_event_bytes = 0");
    assert!(matches!(
        load_bootstrap(&zero),
        Err(BootstrapError::InvalidLimit {
            name: "max_sse_event_bytes"
        })
    ));
}

#[test]
fn model_metadata_and_typed_constraints_are_validated() {
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
        Err(RegistryError::InconsistentReasoningMetadata { .. })
    ));

    let mut invalid_levels = definition("test", "code-primary", "test-model");
    invalid_levels.models[0].reasoning_levels = vec![ReasoningLevel::Low];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_levels),
        Err(RegistryError::InconsistentReasoningMetadata { .. })
    ));

    let mut duplicate_levels = definition("test", "code-primary", "test-model");
    duplicate_levels.models[0].supported_parameters = vec!["reasoning".to_owned()];
    duplicate_levels.models[0].reasoning = ReasoningSupport::Supported;
    duplicate_levels.models[0].reasoning_levels = vec![ReasoningLevel::High, ReasoningLevel::High];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate_levels),
        Err(RegistryError::InconsistentReasoningMetadata { .. })
    ));
}

#[test]
fn deployment_constraints_only_reduce_model_metadata() {
    let mut definition = definition("test", "code-primary", "test-model");
    definition.models[0].supported_parameters =
        vec!["max_tokens".to_owned(), "reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.deployments[0].model_constraints.context_length =
        ModelContextLength::new(Some(64_000), Some(4_096));
    definition.deployments[0].model_constraints.reasoning = Some(ReasoningSupport::Unsupported);
    definition.deployments[0]
        .model_constraints
        .disabled_parameters = vec!["reasoning".to_owned()];

    let registry = build_registry(bootstrap(BOOTSTRAP), definition).unwrap();
    let base = registry.model("openai/test-model").unwrap();
    let effective = registry.deployment("openai-main").unwrap().model();

    assert_eq!(base.context_length().output_tokens(), Some(8_192));
    assert_eq!(base.reasoning(), ReasoningSupport::Supported);
    assert_eq!(effective.context_length().output_tokens(), Some(4_096));
    assert_eq!(effective.reasoning(), ReasoningSupport::Unsupported);
    assert_eq!(effective.supported_parameters(), ["max_tokens"]);
}

#[test]
fn deployment_constraints_cannot_widen_model_metadata() {
    let mut widened = definition("test", "code-primary", "test-model");
    widened.deployments[0].model_constraints.context_length =
        ModelContextLength::new(None, Some(8_193));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), widened),
        Err(RegistryError::DeploymentModelConstraintExceedsModelLimit { .. })
    ));

    let mut definition = definition("test", "code-primary", "test-model");
    definition.deployments[0].model_constraints.reasoning = Some(ReasoningSupport::Supported);
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), definition),
        Err(RegistryError::DeploymentModelConstraintWidensModelMetadata { .. })
    ));
}

#[test]
fn endpoint_registration_accepts_safe_prefixes_and_rejects_unsafe_urls() {
    for base_url in [
        "https://api.openai.com/openai",
        "https://api.openai.com/openai/",
    ] {
        let mut definition = definition("test", "code-primary", "test-model");
        definition.deployments[0].base_url = base_url.to_owned();
        let registry = build_registry(bootstrap(BOOTSTRAP), definition).unwrap();
        assert_eq!(
            registry
                .deployment("openai-main")
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
        definition.deployments[0].base_url = base_url.to_owned();
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
    unknown.aliases[0].candidates = vec!["missing".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), unknown),
        Err(RegistryError::UnknownReference {
            entity: "alias",
            ..
        })
    ));

    let mut duplicate_candidate = definition("test", "code-primary", "test-model");
    duplicate_candidate.aliases = vec![AliasDefinition {
        name: "code-primary".to_owned(),
        candidates: vec!["openai-main".to_owned(), "openai-main".to_owned()],
    }];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate_candidate),
        Err(RegistryError::DuplicateAliasCandidate { .. })
    ));
}

#[test]
fn registry_rejects_capability_elevation_and_invalid_credential_locator() {
    let mut elevation = definition("test", "code-primary", "test-model");
    elevation.deployments[0].capabilities.responses.background = true;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), elevation),
        Err(RegistryError::CapabilityElevation { .. })
    ));

    let mut locator = definition("test", "code-primary", "test-model");
    locator.providers[0].credential.environment_variable = "not-valid".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), locator),
        Err(RegistryError::InvalidCredentialLocator { .. })
    ));
}
