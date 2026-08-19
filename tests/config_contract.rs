//! Verifies bootstrap, registry compilation, reference integrity, and endpoint/credential boundaries.

mod support;

use std::{collections::BTreeSet, path::Path};

use openbridge::{
    config::{BootstrapConfigError, parse_bootstrap_config},
    core::{ExecutableResponsesState, OperationKind, ResponsesAffinity, StorageSupport},
    provider::{CredentialKind, ProviderKind},
    providers::build_compiled_registry_with_active_pools,
    registry::{
        IgnorableGenerationParameter, ModelContextLength, ModelLifecycle, ModelLifecycleStatus,
        NonStreamingConversion, PublicModelConfig, ReasoningLevel, ReasoningLevelMapping,
        ReasoningProfile, ReasoningSupport, RegistryError, UpstreamApiCapabilities,
        UpstreamStreamingPolicy, build_registry,
    },
};

use support::{BOOTSTRAP, bootstrap, definition};

#[test]
fn bootstrap_and_code_registry_resolve_runtime_boundaries() {
    let policy = bootstrap(BOOTSTRAP);
    let registry = build_registry(
        policy.clone(),
        definition("test-1", "code-primary", "test-model"),
    )
    .unwrap();

    // Preserve the local-only process and bounded replay relationships without copying every fixture value.
    assert!(registry.listen().ip().is_loopback());
    assert_ne!(policy.users_file(), policy.upstream_credentials_file());
    assert!(
        registry.limits().max_replay_body_bytes() <= registry.limits().max_request_body_bytes()
    );
    assert!(
        registry.limits().max_request_body_bytes()
            <= registry.limits().max_json_response_body_bytes()
    );
    assert!(!registry.http_client().connect_timeout().is_zero());

    // Resolve the configured target through a trusted credential pool and HTTPS endpoint.
    let target = registry.upstream_target("openai-main").unwrap();
    assert!(
        registry
            .credential_pool(target.credential_pool_id())
            .is_some()
    );
    assert_eq!(target.endpoint_base().scheme(), "https");
}

#[test]
fn shared_generation_fixture_denies_optional_api_capabilities_by_default() {
    let definition = definition("minimal-fixture", "public-model", "upstream-model");
    let UpstreamApiCapabilities::ChatCompletions(chat) =
        definition.upstream_targets[0].upstream_apis[0].capabilities
    else {
        panic!("the shared fixture must keep one minimal Chat API");
    };
    let UpstreamApiCapabilities::Responses(responses) =
        definition.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("the shared fixture must keep one minimal Responses API");
    };

    assert!(!chat.streaming);
    assert!(!chat.stream_usage);
    assert!(chat.function_tools.is_none());
    assert!(!responses.streaming);
    assert!(!responses.terminal_usage);
    assert!(responses.function_tools.is_none());
}

#[test]
fn bootstrap_http_logging_switches_fallback_off_and_enable_independently() {
    // Keep every content-bearing local HTTP event disabled when the optional table is absent.
    let disabled = parse_bootstrap_config(BOOTSTRAP).unwrap();
    assert!(!disabled.http_logging().request_headers());
    assert!(!disabled.http_logging().request_body());
    assert!(!disabled.http_logging().response_headers());
    assert!(!disabled.http_logging().response_body());

    // Enable an arbitrary subset without coupling header and body decisions.
    let enabled = format!(
        "{BOOTSTRAP}\n[logging]\nhttp_jsonl_directory = \"/var/lib/openbridge/http-logs\"\nrequest_headers = true\nresponse_body = true\n"
    );
    let enabled = parse_bootstrap_config(&enabled).unwrap();
    assert!(enabled.http_logging().request_headers());
    assert!(!enabled.http_logging().request_body());
    assert!(!enabled.http_logging().response_headers());
    assert!(enabled.http_logging().response_body());
    assert!(enabled.http_logging().is_enabled());
    assert_eq!(
        enabled.http_logging().http_jsonl_directory(),
        Some(Path::new("/var/lib/openbridge/http-logs"))
    );

    // Reject misspelled or future logging policy instead of silently ignoring it.
    let unknown = format!(
        "{BOOTSTRAP}\n[logging]\nhttp_jsonl_directory = \"/var/lib/openbridge/http-logs\"\nrequest_headers = true\nrequest_body = false\nresponse_headers = false\nresponse_body = false\ninclude_credentials = true\n"
    );
    assert!(matches!(
        parse_bootstrap_config(&unknown),
        Err(BootstrapConfigError::Parse)
    ));
}

#[test]
fn registry_rejects_duplicate_upstream_operations() {
    let mut duplicate = definition("test", "code-primary", "test-model");
    duplicate.upstream_targets[0].upstream_apis[1].capabilities =
        duplicate.upstream_targets[0].upstream_apis[0].capabilities;

    let error = build_registry(bootstrap(BOOTSTRAP), duplicate).unwrap_err();

    assert!(matches!(
        error,
        RegistryError::DuplicateUpstreamOperation {
            upstream_target,
            upstream_operation: OperationKind::ChatCompletions,
        } if upstream_target == "openai-main"
    ));
}

#[test]
fn registry_rejects_a_route_for_an_absent_upstream_operation() {
    let mut absent = definition("test", "code-primary", "test-model");
    absent.upstream_targets[0]
        .upstream_apis
        .retain(|api| api.capabilities.operation() != OperationKind::Responses);

    let error = build_registry(bootstrap(BOOTSTRAP), absent).unwrap_err();

    assert!(matches!(
        error,
        RegistryError::UnknownReference {
            entity: "route",
            id,
            target: "upstream operation",
            ..
        } if id == "public-responses"
    ));
}

#[test]
fn bootstrap_rejects_unknown_fields_non_loopback_and_zero_limits() {
    let old_schema = BOOTSTRAP.replace("schema_version = 2", "schema_version = 1");
    assert!(matches!(
        parse_bootstrap_config(&old_schema),
        Err(BootstrapConfigError::UnsupportedSchema { actual: 1 })
    ));

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

    // Require an independent non-zero JSON response budget instead of deriving it from requests.
    let missing_response_limit = BOOTSTRAP.replace("max_json_response_body_bytes = 16777216\n", "");
    assert!(matches!(
        parse_bootstrap_config(&missing_response_limit),
        Err(BootstrapConfigError::Parse)
    ));
    let zero_response_limit = BOOTSTRAP.replace(
        "max_json_response_body_bytes = 16777216",
        "max_json_response_body_bytes = 0",
    );
    assert!(matches!(
        parse_bootstrap_config(&zero_response_limit),
        Err(BootstrapConfigError::InvalidLimit {
            name: "max_json_response_body_bytes"
        })
    ));

    // Require a separate non-zero replay budget bounded by the request hard limit.
    let missing_replay_limit = BOOTSTRAP.replace("max_replay_body_bytes = 262144\n", "");
    assert!(matches!(
        parse_bootstrap_config(&missing_replay_limit),
        Err(BootstrapConfigError::Parse)
    ));
    let zero_replay_limit = BOOTSTRAP.replace(
        "max_replay_body_bytes = 262144",
        "max_replay_body_bytes = 0",
    );
    assert!(matches!(
        parse_bootstrap_config(&zero_replay_limit),
        Err(BootstrapConfigError::InvalidLimit {
            name: "max_replay_body_bytes"
        })
    ));
    let replay_over_request = BOOTSTRAP.replace(
        "max_replay_body_bytes = 262144",
        "max_replay_body_bytes = 1048577",
    );
    assert!(matches!(
        parse_bootstrap_config(&replay_over_request),
        Err(BootstrapConfigError::ReplayLimitExceedsRequest {
            replay: 1_048_577,
            request: 1_048_576
        })
    ));
}

#[test]
fn active_general_generation_interfaces_require_non_blank_default_instructions() {
    // Keep the optional field absent when no credential pool activates a general Generation interface.
    let without_instructions = BOOTSTRAP.replace(
        "default_instructions = \"You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed.\"\n",
        "",
    );
    build_compiled_registry_with_active_pools(
        parse_bootstrap_config(&without_instructions).unwrap(),
        &BTreeSet::new(),
    )
    .expect("a deployment without an active general Generation interface needs no default");

    // Reject missing, empty, and whitespace-only values for any active general Generation interface.
    for document in [
        without_instructions,
        BOOTSTRAP.replace(
            "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed.",
            "",
        ),
        BOOTSTRAP.replace(
            "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed.",
            "   ",
        ),
    ] {
        assert!(matches!(
            build_registry(
                parse_bootstrap_config(&document).unwrap(),
                definition("instructions-test", "public-model", "upstream-model"),
            ),
            Err(RegistryError::MissingDefaultInstructions)
        ));
    }

    // The unpublished ChatGPT-only field is replaced outright by the project-wide field.
    let legacy = BOOTSTRAP.replace("default_instructions", "chatgpt_instructions");
    assert!(matches!(
        parse_bootstrap_config(&legacy),
        Err(BootstrapConfigError::Parse)
    ));
}

#[test]
fn bootstrap_accepts_explicit_otlp_trace_export_and_rejects_unsafe_url_shapes() {
    // Keep trace export fully disabled when the optional telemetry table is absent.
    assert!(
        parse_bootstrap_config(BOOTSTRAP)
            .unwrap()
            .otlp_http_trace_export()
            .is_none()
    );

    // Accept startup-owned loopback, non-loopback IP, and DNS collector bases.
    for (endpoint, normalized) in [
        ("http://127.0.0.1:4318", "http://127.0.0.1:4318/"),
        ("http://[::1]:4318", "http://[::1]:4318/"),
        ("http://192.0.2.1:4318", "http://192.0.2.1:4318/"),
        (
            "http://collector.example:4318",
            "http://collector.example:4318/",
        ),
        ("http://localhost:4318", "http://localhost:4318/"),
    ] {
        let enabled =
            format!("{BOOTSTRAP}\n[telemetry.traces]\notlp_http_endpoint = \"{endpoint}\"\n");
        assert_eq!(
            parse_bootstrap_config(&enabled)
                .unwrap()
                .otlp_http_trace_export()
                .unwrap()
                .endpoint()
                .as_str(),
            normalized
        );
    }

    // Reject URL shapes that could smuggle credentials, routing data, or a different protocol.
    for endpoint in [
        "https://127.0.0.1:4318",
        "file:///tmp/collector",
        "http://",
        "http://user:synthetic-secret@127.0.0.1:4318",
        "http://127.0.0.1:4318/custom",
        "http://127.0.0.1:4318?tenant=synthetic",
        "http://127.0.0.1:4318#synthetic",
    ] {
        let invalid =
            format!("{BOOTSTRAP}\n[telemetry.traces]\notlp_http_endpoint = \"{endpoint}\"\n");
        assert!(matches!(
            parse_bootstrap_config(&invalid),
            Err(BootstrapConfigError::InvalidOtlpHttpTraceEndpoint)
        ));
    }

    // Deny exporter headers and all other unowned telemetry policy at the document boundary.
    let custom_header = format!(
        "{BOOTSTRAP}\n[telemetry.traces]\notlp_http_endpoint = \"http://127.0.0.1:4318\"\nheaders = {{ authorization = \"synthetic-secret\" }}\n"
    );
    assert!(matches!(
        parse_bootstrap_config(&custom_header),
        Err(BootstrapConfigError::Parse)
    ));
}

#[test]
fn bootstrap_accepts_explicit_otlp_metrics_export_and_rejects_unsafe_url_shapes() {
    // Keep metrics export fully disabled when the optional telemetry table is absent.
    assert!(
        parse_bootstrap_config(BOOTSTRAP)
            .unwrap()
            .otlp_http_metrics_export()
            .is_none()
    );

    // Accept startup-owned loopback, non-loopback IP, and DNS collector bases.
    for (endpoint, normalized) in [
        ("http://127.0.0.1:4318", "http://127.0.0.1:4318/"),
        ("http://[::1]:4318", "http://[::1]:4318/"),
        ("http://192.0.2.1:4318", "http://192.0.2.1:4318/"),
        (
            "http://collector.example:4318",
            "http://collector.example:4318/",
        ),
        ("http://localhost:4318", "http://localhost:4318/"),
    ] {
        let enabled =
            format!("{BOOTSTRAP}\n[telemetry.metrics]\notlp_http_endpoint = \"{endpoint}\"\n");
        assert_eq!(
            parse_bootstrap_config(&enabled)
                .unwrap()
                .otlp_http_metrics_export()
                .unwrap()
                .endpoint()
                .as_str(),
            normalized
        );
    }

    // Reject URL shapes that could smuggle credentials, routing data, or a different protocol.
    for endpoint in [
        "https://127.0.0.1:4318",
        "file:///tmp/collector",
        "http://",
        "http://user:synthetic-secret@127.0.0.1:4318",
        "http://127.0.0.1:4318/custom",
        "http://127.0.0.1:4318?tenant=synthetic",
        "http://127.0.0.1:4318#synthetic",
    ] {
        let invalid =
            format!("{BOOTSTRAP}\n[telemetry.metrics]\notlp_http_endpoint = \"{endpoint}\"\n");
        assert!(matches!(
            parse_bootstrap_config(&invalid),
            Err(BootstrapConfigError::InvalidOtlpHttpMetricsEndpoint)
        ));
    }

    // Deny exporter headers and all other unowned telemetry policy at the document boundary.
    let custom_header = format!(
        "{BOOTSTRAP}\n[telemetry.metrics]\notlp_http_endpoint = \"http://127.0.0.1:4318\"\nheaders = {{ authorization = \"synthetic-secret\" }}\n"
    );
    assert!(matches!(
        parse_bootstrap_config(&custom_header),
        Err(BootstrapConfigError::Parse)
    ));
}

#[test]
fn model_config_and_typed_rules_are_validated() {
    let mut invalid = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut invalid.models[0]).context_length =
        ModelContextLength::new(None, None, Some(0));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid),
        Err(RegistryError::InvalidModelContextLength { .. })
    ));

    let mut inconsistent_context = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut inconsistent_context.models[0]).context_length =
        ModelContextLength::new(Some(4_096), None, Some(8_192));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), inconsistent_context),
        Err(RegistryError::InconsistentModelContextLength { .. })
    ));

    let mut duplicate = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut duplicate.models[0]).supported_parameters =
        vec!["tools".to_owned(), "tools".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::DuplicateSupportedParameter { .. })
    ));

    let mut unknown_parameter = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut unknown_parameter.models[0]).supported_parameters =
        vec!["future_parameter".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), unknown_parameter),
        Err(RegistryError::InvalidSupportedParameter { parameter, .. })
            if parameter == "future_parameter"
    ));

    let mut reasoning_alias = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut reasoning_alias.models[0]).supported_parameters =
        vec!["reasoning".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), reasoning_alias),
        Err(RegistryError::InvalidSupportedParameter { parameter, .. })
            if parameter == "reasoning"
    ));

    let mut reasoning_effort_alias = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut reasoning_effort_alias.models[0]).supported_parameters =
        vec!["reasoning_effort".to_owned()];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), reasoning_effort_alias),
        Err(RegistryError::InvalidSupportedParameter { parameter, .. })
            if parameter == "reasoning_effort"
    ));

    let reasoning = ReasoningProfile::supported([ReasoningLevel::High, ReasoningLevel::High]);
    assert_eq!(reasoning.levels(), [ReasoningLevel::High]);
}

#[test]
fn canonical_and_provider_model_id_layers_are_validated_separately() {
    let mut invalid_canonical = definition("test", "code-primary", "test-model");
    invalid_canonical.models[0].id = "test-model".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_canonical),
        Err(RegistryError::InvalidNamespacedModelId { field: "id", .. })
    ));

    let mut invalid_provider_model = definition("test", "code-primary", "test-model");
    invalid_provider_model.upstream_targets[0].provider_model = "openai/test/model".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_provider_model),
        Err(RegistryError::InvalidNamespacedModelId {
            field: "provider_model",
            ..
        })
    ));

    let mut mismatched_provider_model = definition("test", "code-primary", "test-model");
    mismatched_provider_model.upstream_targets[0].provider_model = "chatgpt/test-model".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), mismatched_provider_model),
        Err(RegistryError::ProviderModelMismatch { .. })
    ));
}

#[test]
fn upstream_api_rules_only_reduce_model_info() {
    let mut definition = definition("test", "code-primary", "test-model");
    let profile = support::generation_profile_mut(&mut definition.models[0]);
    profile.supported_parameters = vec!["max_tokens".to_owned()];
    profile.reasoning = ReasoningProfile::supported([ReasoningLevel::High]);
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .context_length = ModelContextLength::new(Some(64_000), None, Some(4_096));
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .reasoning = Some(ReasoningProfile::Unsupported);

    let registry = build_registry(bootstrap(BOOTSTRAP), definition).unwrap();
    let base = registry.model("openai/test-model").unwrap();
    let effective = registry
        .upstream_target("openai-main")
        .unwrap()
        .upstream_api(OperationKind::ChatCompletions)
        .unwrap()
        .model();

    assert_eq!(base.context_length().output_tokens(), Some(8_192));
    assert_eq!(base.reasoning_support(), ReasoningSupport::Supported);
    assert_eq!(effective.context_length().output_tokens(), Some(4_096));
    assert_eq!(effective.reasoning_support(), ReasoningSupport::Unsupported);
    assert_eq!(effective.supported_parameters(), ["max_tokens"]);
}

#[test]
fn upstream_api_ignored_parameters_remain_accepted_but_are_validated() {
    let mut accepted = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut accepted.models[0]).supported_parameters =
        vec!["seed".to_owned(), "temperature".to_owned()];
    accepted.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];

    let registry = build_registry(bootstrap(BOOTSTRAP), accepted).unwrap();
    let api = registry
        .upstream_target("openai-main")
        .unwrap()
        .upstream_api(OperationKind::ChatCompletions)
        .unwrap();
    assert_eq!(api.model().supported_parameters(), ["seed", "temperature"]);
    assert!(api.ignores_generation_parameter(IgnorableGenerationParameter::Temperature));
    assert!(!api.ignores_generation_parameter(IgnorableGenerationParameter::Seed));

    let info = serde_json::to_value(registry.public_model("code-primary").unwrap().info()).unwrap();
    let parameters = info["interfaces"]["chat_completions"]["supported_parameters"]
        .as_array()
        .unwrap();
    assert!(parameters.iter().any(|value| value == "seed"));
    assert!(parameters.iter().any(|value| value == "temperature"));

    let mut undeclared = definition("test", "code-primary", "test-model");
    undeclared.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), undeclared),
        Err(RegistryError::UpstreamApiModelRuleIgnoresUnknownParameter { .. })
    ));

    let mut duplicate = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut duplicate.models[0]).supported_parameters =
        vec!["temperature".to_owned()];
    duplicate.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![
        IgnorableGenerationParameter::Temperature,
        IgnorableGenerationParameter::Temperature,
    ];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));

    let mut contradictory = definition("test", "code-primary", "test-model");
    support::generation_profile_mut(&mut contradictory.models[0]).supported_parameters =
        vec!["temperature".to_owned()];
    contradictory.upstream_targets[0].upstream_apis[0]
        .model_rules
        .disabled_parameters = vec!["temperature".to_owned()];
    contradictory.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), contradictory),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));
}

#[test]
fn upstream_api_rules_cannot_widen_model_info() {
    let mut widened = definition("test", "code-primary", "test-model");
    widened.upstream_targets[0].upstream_apis[0]
        .model_rules
        .context_length = ModelContextLength::new(None, None, Some(8_193));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), widened),
        Err(RegistryError::UpstreamApiModelLimitExceedsModel { .. })
    ));

    let mut widened_reasoning = definition("test", "code-primary", "test-model");
    widened_reasoning.upstream_targets[0].upstream_apis[0]
        .model_rules
        .reasoning = Some(ReasoningProfile::supported([ReasoningLevel::High]));
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), widened_reasoning),
        Err(RegistryError::UpstreamApiModelRuleWidensModel { .. })
    ));

    let mut inconsistent = definition("test", "code-primary", "test-model");
    inconsistent.upstream_targets[0].upstream_apis[0]
        .model_rules
        .context_length = ModelContextLength::new(Some(4_096), None, None);
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), inconsistent),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));
}

#[test]
fn public_model_identity_and_lifecycle_are_validated() {
    let mut invalid_id = definition("test", "code-primary", "test-model");
    invalid_id.public_models[0].id = "provider/model".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_id),
        Err(RegistryError::InvalidPublicModelId { .. })
    ));

    let mut missing_created = definition("test", "code-primary", "test-model");
    missing_created.public_models[0].created = 0;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), missing_created),
        Err(RegistryError::InvalidPublicModelCreated { .. })
    ));

    let mut blank_name = definition("test", "code-primary", "test-model");
    blank_name.public_models[0].display_name = "  ".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), blank_name),
        Err(RegistryError::BlankPublicModelField { .. })
    ));

    let mut invalid_lifecycle = definition("test", "code-primary", "test-model");
    invalid_lifecycle.public_models[0].lifecycle = ModelLifecycle {
        status: ModelLifecycleStatus::Deprecated,
        deprecated_at: None,
        retired_at: None,
    };
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_lifecycle),
        Err(RegistryError::InvalidPublicModelLifecycle { .. })
    ));
}

#[test]
fn reasoning_level_mappings_are_validated_at_registry_build_time() {
    let configured = |mapping: ReasoningLevelMapping| {
        let mut definition = definition("test", "code-primary", "test-model");
        support::generation_profile_mut(&mut definition.models[0]).reasoning =
            ReasoningProfile::supported([ReasoningLevel::XHigh]);
        definition.upstream_targets[0].upstream_apis[1]
            .model_rules
            .reasoning_level_mappings = vec![mapping];
        definition
    };

    // Reject undeclared canonical Model mappings and invalid upstream wire values.
    let unknown_source = configured(ReasoningLevelMapping {
        downstream: ReasoningLevel::High,
        upstream: "max".to_owned(),
    });
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), unknown_source),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));
    let invalid_target = configured(ReasoningLevelMapping {
        downstream: ReasoningLevel::XHigh,
        upstream: "MAX!".to_owned(),
    });
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid_target),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));

    // One Upstream API must not declare ambiguous targets for the same downstream level.
    let mut duplicate = configured(ReasoningLevelMapping {
        downstream: ReasoningLevel::XHigh,
        upstream: "max".to_owned(),
    });
    duplicate.upstream_targets[0].upstream_apis[1]
        .model_rules
        .reasoning_level_mappings
        .push(ReasoningLevelMapping {
            downstream: ReasoningLevel::XHigh,
            upstream: "high".to_owned(),
        });
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));
}

#[test]
fn provider_instance_registration_owns_and_validates_endpoint_bases() {
    for base_url in [
        "https://api.openai.com/openai",
        "https://api.openai.com/openai/",
    ] {
        let mut definition = definition("test", "code-primary", "test-model");
        definition.provider_instances[0].base_url = base_url.to_owned();
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
        definition.provider_instances[0].base_url = base_url.to_owned();
        assert!(matches!(
            build_registry(bootstrap(BOOTSTRAP), definition),
            Err(RegistryError::InvalidProviderBaseUrl { .. })
        ));
    }
}

#[test]
fn enabled_http_content_logging_requires_an_absolute_jsonl_directory() {
    for switch in [
        "request_headers",
        "request_body",
        "response_headers",
        "response_body",
    ] {
        let missing = format!("{BOOTSTRAP}\n[logging]\n{switch} = true\n");
        assert!(matches!(
            parse_bootstrap_config(&missing),
            Err(BootstrapConfigError::MissingHttpJsonlDirectory)
        ));
    }

    let relative = format!(
        "{BOOTSTRAP}\n[logging]\nhttp_jsonl_directory = \"relative/http-logs\"\nrequest_headers = true\nrequest_body = false\nresponse_headers = false\nresponse_body = false\n"
    );
    assert!(matches!(
        parse_bootstrap_config(&relative),
        Err(BootstrapConfigError::RelativeHttpJsonlDirectory)
    ));

    let disabled = format!(
        "{BOOTSTRAP}\n[logging]\nrequest_headers = false\nrequest_body = false\nresponse_headers = false\nresponse_body = false\n"
    );
    let disabled = parse_bootstrap_config(&disabled).unwrap();
    assert!(!disabled.http_logging().is_enabled());
    assert_eq!(disabled.http_logging().http_jsonl_directory(), None);
}

#[test]
fn provider_instances_require_unique_known_ids_and_allow_multiple_deployments_per_kind() {
    let mut blank = definition("test", "code-primary", "test-model");
    blank.provider_instances[0].id = "   ".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), blank),
        Err(RegistryError::BlankProviderInstanceId)
    ));

    let mut duplicate = definition("test", "code-primary", "test-model");
    duplicate
        .provider_instances
        .push(duplicate.provider_instances[0].clone());
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::DuplicateId {
            entity: "provider instance",
            ..
        })
    ));

    let mut unknown = definition("test", "code-primary", "test-model");
    unknown.upstream_targets[0].provider_instance = "missing".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), unknown),
        Err(RegistryError::UnknownReference {
            target: "provider instance",
            ..
        })
    ));

    let mut regional = definition("test", "code-primary", "test-model");
    let mut regional_instance = regional.provider_instances[0].clone();
    regional_instance.id = "openai-eu".to_owned();
    regional_instance.base_url = "https://eu.api.openai.com".to_owned();
    regional.provider_instances.push(regional_instance);
    let mut regional_target = regional.upstream_targets[0].clone();
    regional_target.id = "openai-eu-target".to_owned();
    regional_target.provider_instance = "openai-eu".to_owned();
    regional.upstream_targets.push(regional_target);

    let registry = build_registry(bootstrap(BOOTSTRAP), regional).unwrap();
    assert_eq!(
        registry
            .upstream_target("openai-eu-target")
            .unwrap()
            .endpoint_base()
            .as_str(),
        "https://eu.api.openai.com/"
    );
    assert_eq!(
        registry.provider_instance("openai-eu").unwrap().kind(),
        ProviderKind::OpenAi
    );
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
        id: "code-primary".to_owned(),
        created: 1_785_715_200,
        display_name: "Code Primary".to_owned(),
        description: None,
        lifecycle: ModelLifecycle::active(),
        reasoning_level_policy: openbridge::registry::ReasoningLevelPolicy::Strict,
        routes: vec!["public-chat".to_owned(), "public-chat".to_owned()],
    }];
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate_candidate),
        Err(RegistryError::DuplicatePublicModelRoute { .. })
    ));
}

#[test]
fn registry_rejects_capability_elevation_and_unsupported_credential_kind() {
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

    // Reject each independent executable Responses state axis when the Provider ceiling is stateless.
    for state in [
        ExecutableResponsesState::new(StorageSupport::Supported, ResponsesAffinity::TargetBound),
        ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::TargetBoundContinuation,
        ),
    ] {
        let mut elevation = definition("test", "code-primary", "test-model");
        elevation.provider_instances[0].kind = ProviderKind::LongCat;
        elevation.credential_pools[0].provider = ProviderKind::LongCat;
        elevation.upstream_targets[0].provider_model =
            ProviderKind::LongCat.routing_model_id(&elevation.upstream_targets[0].canonical_model);
        let UpstreamApiCapabilities::Responses(capabilities) =
            &mut elevation.upstream_targets[0].upstream_apis[1].capabilities
        else {
            panic!("second synthetic API must be Responses");
        };
        capabilities.state = state;
        assert!(matches!(
            build_registry(bootstrap(BOOTSTRAP), elevation),
            Err(RegistryError::CapabilityElevation {
                upstream_operation: OperationKind::Responses,
                ..
            })
        ));
    }

    // Accept the combined executable state when both axes stay within the OpenAI Provider ceiling.
    let mut within_ceiling = definition("test", "code-primary", "test-model");
    let UpstreamApiCapabilities::Responses(capabilities) =
        &mut within_ceiling.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("second synthetic API must be Responses");
    };
    capabilities.state = ExecutableResponsesState::new(
        StorageSupport::Supported,
        ResponsesAffinity::TargetBoundContinuation,
    );
    build_registry(bootstrap(BOOTSTRAP), within_ceiling)
        .expect("OpenAI state axes must remain within its combined Provider ceiling");

    let mut absent_operation = definition("test", "code-primary", "test-model");
    absent_operation.provider_instances[0].kind = ProviderKind::ChatGpt;
    absent_operation.credential_pools[0].provider = ProviderKind::ChatGpt;
    absent_operation.credential_pools[0].kind = CredentialKind::OAuth2BearerAccessToken;
    absent_operation.upstream_targets[0].provider_model = "chatgpt/test-model".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), absent_operation),
        Err(RegistryError::CapabilityElevation {
            upstream_operation: OperationKind::ChatCompletions,
            ..
        })
    ));

    let mut oauth = definition("test", "code-primary", "test-model");
    oauth.credential_pools[0].kind = CredentialKind::OAuth2BearerAccessToken;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), oauth),
        Err(RegistryError::UnsupportedCredentialPoolKind { .. })
    ));
}

#[test]
fn registry_rejects_responses_sse_buffering_on_non_responses_apis() {
    // Attach the typed Responses-only conversion to a Chat Completions Upstream API.
    let mut invalid = definition("test", "code-primary", "test-model");
    invalid.upstream_targets[0].upstream_apis[0].streaming_policy =
        UpstreamStreamingPolicy::Required {
            non_streaming: NonStreamingConversion::BufferResponsesSse,
        };

    // Fail startup before an incompatible stream decoder can enter request planning.
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid),
        Err(RegistryError::InvalidUpstreamStreamingPolicy { .. })
    ));
}

#[test]
fn registry_rejects_required_streaming_when_generation_streaming_is_disabled() {
    // Declare a streaming-only policy on an API whose generation capability rejects streaming.
    let mut invalid = definition("test", "code-primary", "test-model");
    let upstream_api = &mut invalid.upstream_targets[0].upstream_apis[0];
    let UpstreamApiCapabilities::ChatCompletions(capabilities) = &mut upstream_api.capabilities
    else {
        panic!("first synthetic API must be Chat Completions");
    };
    capabilities.streaming = false;
    upstream_api.streaming_policy = UpstreamStreamingPolicy::Required {
        non_streaming: NonStreamingConversion::Disabled,
    };

    // Fail startup before the planner can force a request mode the API says it cannot serve.
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), invalid),
        Err(RegistryError::InvalidUpstreamStreamingPolicy { .. })
    ));
}

#[test]
fn registry_rejects_invalid_credential_pool_identity_and_target_ownership() {
    // Pool IDs must be non-empty and unique; a target may reference only a known pool for the same Provider.
    let mut blank = definition("test", "code-primary", "test-model");
    blank.credential_pools[0].id = "   ".to_owned();
    blank.upstream_targets[0].credential_pool = "   ".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), blank),
        Err(RegistryError::BlankCredentialPoolId)
    ));

    let mut unknown = definition("test", "code-primary", "test-model");
    unknown.upstream_targets[0].credential_pool = "missing".to_owned();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), unknown),
        Err(RegistryError::UnknownReference {
            target: "credential pool",
            ..
        })
    ));

    let mut mismatch = definition("test", "code-primary", "test-model");
    mismatch.credential_pools[0].provider = ProviderKind::LongCat;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), mismatch),
        Err(RegistryError::CredentialPoolProviderMismatch { .. })
    ));

    let mut duplicate = definition("test", "code-primary", "test-model");
    duplicate
        .credential_pools
        .push(duplicate.credential_pools[0].clone());
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), duplicate),
        Err(RegistryError::DuplicateId {
            entity: "credential pool",
            ..
        })
    ));
}
