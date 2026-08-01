#![allow(dead_code)]

use std::time::Duration;

use openbridge::{
    config::{BootstrapPolicy, load_bootstrap},
    core::{CapabilitySet, Protocol, ProtocolCapabilities, ResponsesCapabilities},
    pipeline::{RouteError, RoutePlan, analyze_request, plan_request},
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialDefinition, ModelConstraints, ModelContextLength, NativeOfferingCapabilities,
        NativeOfferingDefinition, NativeTransport, PublicModelDefinition, RealModelDefinition,
        ReasoningSupport, RegistryDefinition, RegistrySnapshot, ServingRouteDefinition,
        ServingRouteMode, StatePolicy, UpstreamTargetDefinition, build_registry,
    },
};

pub const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

pub fn bootstrap(document: &str) -> BootstrapPolicy {
    load_bootstrap(document).expect("test bootstrap must be valid")
}

pub fn capabilities() -> CapabilitySet {
    CapabilitySet {
        chat_completions: ProtocolCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            previous_response_id: false,
            background: false,
        },
    }
}

pub fn definition(version: &str, alias: &str, upstream_model: &str) -> RegistryDefinition {
    RegistryDefinition {
        version: version.to_owned(),
        real_models: vec![RealModelDefinition {
            id: "openai/test-model".to_owned(),
            name: "Test model".to_owned(),
            description: Some("Model used by integration tests.".to_owned()),
            context_length: ModelContextLength::new(Some(128_000), Some(8_192)),
            supported_parameters: Vec::new(),
            reasoning: ReasoningSupport::Unknown,
            reasoning_levels: Vec::new(),
        }],
        upstream_targets: vec![UpstreamTargetDefinition {
            id: "openai-main".to_owned(),
            provider: ProviderKind::OpenAi,
            real_model: "openai/test-model".to_owned(),
            base_url: "https://api.openai.com".to_owned(),
            credential: CredentialDefinition {
                id: "openai-primary".to_owned(),
                kind: CredentialKind::ApiKey,
                environment_variable: "OPENAI_API_KEY".to_owned(),
            },
            quota_scope: None,
            fault_domain: None,
            request_timeout: Duration::from_secs(120),
            enabled: true,
            offerings: vec![
                NativeOfferingDefinition {
                    id: "chat".to_owned(),
                    protocol: Protocol::ChatCompletions,
                    upstream_model: upstream_model.to_owned(),
                    endpoint_profile: "public-api".to_owned(),
                    transport: NativeTransport::HttpJsonSse,
                    model_constraints: ModelConstraints::default(),
                    capabilities: NativeOfferingCapabilities::ChatCompletions(
                        capabilities().chat_completions,
                    ),
                    state_policy: StatePolicy::Stateless,
                },
                NativeOfferingDefinition {
                    id: "responses".to_owned(),
                    protocol: Protocol::Responses,
                    upstream_model: upstream_model.to_owned(),
                    endpoint_profile: "public-api".to_owned(),
                    transport: NativeTransport::HttpJsonSse,
                    model_constraints: ModelConstraints::default(),
                    capabilities: NativeOfferingCapabilities::Responses(capabilities().responses),
                    state_policy: StatePolicy::ProviderBound,
                },
            ],
        }],
        serving_routes: vec![
            ServingRouteDefinition {
                id: "public-chat".to_owned(),
                upstream_target: "openai-main".to_owned(),
                offering: "chat".to_owned(),
                downstream_protocol: Protocol::ChatCompletions,
                mode: ServingRouteMode::Native,
            },
            ServingRouteDefinition {
                id: "public-responses".to_owned(),
                upstream_target: "openai-main".to_owned(),
                offering: "responses".to_owned(),
                downstream_protocol: Protocol::Responses,
                mode: ServingRouteMode::Native,
            },
        ],
        public_models: vec![PublicModelDefinition {
            name: alias.to_owned(),
            serving_routes: vec!["public-chat".to_owned(), "public-responses".to_owned()],
        }],
    }
}

pub fn prepare(
    snapshot: &RegistrySnapshot,
    protocol: Protocol,
    body: bytes::Bytes,
) -> Result<RoutePlan, RouteError> {
    let profile = analyze_request(protocol, &body)?;
    plan_request(snapshot, &profile, body)
}

pub fn snapshot(version: &str, alias: &str, upstream_model: &str) -> RegistrySnapshot {
    build_registry(
        bootstrap(BOOTSTRAP),
        definition(version, alias, upstream_model),
    )
    .expect("test registry must be valid")
}
