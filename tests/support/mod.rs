#![allow(dead_code)]

pub mod process_replay;

use std::time::Duration;

use openbridge::{
    config::{BootstrapConfig, parse_bootstrap_config},
    core::{ApiCapabilities, ApiProtocol, EndpointCapabilities, ResponsesCapabilities},
    credential::CredentialStore,
    identity::{UserConfiguration, UserRegistry},
    pipeline::{RequestPlanningError, RoutePlan, analyze_request, plan_request},
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialConfig, ModelConfig, ModelContextLength, PublicModelConfig, ReasoningSupport,
        RegistryConfig, RouteConfig, RouteMode, RuntimeRegistry, StateAffinity, TransportKind,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules, UpstreamTargetConfig,
        build_registry,
    },
};

use std::sync::Arc;

pub const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
users_file = "config/users.toml"
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

pub fn bootstrap(document: &str) -> BootstrapConfig {
    parse_bootstrap_config(document).expect("test bootstrap must be valid")
}

pub fn users_and_credentials(
    api_key: &str,
    registry: &RuntimeRegistry,
    upstream_secret: &str,
) -> (Arc<UserRegistry>, Arc<CredentialStore>) {
    // 解析下游用户，并取得同一个 credential builder。
    let configuration = UserConfiguration::from_toml(&format!(
        r#"
schema_version = 1

[[users]]
id = "test-user"
name = "Test User"
api_key = "{api_key}"
enabled = true
"#
    ))
    .expect("test user registry must be valid");
    let (users, mut credentials) = configuration.into_parts();

    // 为测试 registry 中全部启用 target 注入同一组合成 secret。
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        if target.enabled() {
            credentials
                .insert_upstream(
                    target.kind(),
                    target.credential().id(),
                    secrecy::SecretString::from(upstream_secret.to_owned()),
                )
                .expect("test upstream credential must be unique");
        }
    }
    (Arc::new(users), Arc::new(credentials.build()))
}

pub fn capabilities() -> ApiCapabilities {
    ApiCapabilities {
        chat_completions: EndpointCapabilities {
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

pub fn definition(version: &str, alias: &str, upstream_model: &str) -> RegistryConfig {
    RegistryConfig {
        version: version.to_owned(),
        models: vec![ModelConfig {
            id: "openai/test-model".to_owned(),
            name: "Test model".to_owned(),
            description: Some("Model used by integration tests.".to_owned()),
            context_length: ModelContextLength::new(Some(128_000), Some(8_192)),
            supported_parameters: Vec::new(),
            reasoning: ReasoningSupport::Unknown,
            reasoning_levels: Vec::new(),
        }],
        upstream_targets: vec![UpstreamTargetConfig {
            id: "openai-main".to_owned(),
            provider: ProviderKind::OpenAi,
            model: "openai/test-model".to_owned(),
            base_url: "https://api.openai.com".to_owned(),
            credential: CredentialConfig {
                id: "openai-primary".to_owned(),
                kind: CredentialKind::ApiKey,
                environment_variable: "OPENAI_API_KEY".to_owned(),
            },
            quota_scope: None,
            fault_domain: None,
            request_timeout: Duration::from_secs(120),
            enabled: true,
            upstream_apis: vec![
                UpstreamApiConfig {
                    id: "chat".to_owned(),
                    protocol: ApiProtocol::ChatCompletions,
                    upstream_model: upstream_model.to_owned(),
                    endpoint_profile: "public-api".to_owned(),
                    transport: TransportKind::HttpJsonSse,
                    model_rules: UpstreamApiModelRules::default(),
                    capabilities: UpstreamApiCapabilities::ChatCompletions(
                        capabilities().chat_completions,
                    ),
                    state_affinity: StateAffinity::Unbound,
                },
                UpstreamApiConfig {
                    id: "responses".to_owned(),
                    protocol: ApiProtocol::Responses,
                    upstream_model: upstream_model.to_owned(),
                    endpoint_profile: "public-api".to_owned(),
                    transport: TransportKind::HttpJsonSse,
                    model_rules: UpstreamApiModelRules::default(),
                    capabilities: UpstreamApiCapabilities::Responses(capabilities().responses),
                    state_affinity: StateAffinity::TargetBound,
                },
            ],
        }],
        routes: vec![
            RouteConfig {
                id: "public-chat".to_owned(),
                upstream_target: "openai-main".to_owned(),
                upstream_api: "chat".to_owned(),
                downstream_protocol: ApiProtocol::ChatCompletions,
                mode: RouteMode::Native,
            },
            RouteConfig {
                id: "public-responses".to_owned(),
                upstream_target: "openai-main".to_owned(),
                upstream_api: "responses".to_owned(),
                downstream_protocol: ApiProtocol::Responses,
                mode: RouteMode::Native,
            },
        ],
        public_models: vec![PublicModelConfig {
            name: alias.to_owned(),
            routes: vec!["public-chat".to_owned(), "public-responses".to_owned()],
        }],
    }
}

pub fn prepare(
    registry: &RuntimeRegistry,
    protocol: ApiProtocol,
    body: bytes::Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    let profile = analyze_request(protocol, &body)?;
    plan_request(registry, &profile, body)
}

pub fn registry(version: &str, alias: &str, upstream_model: &str) -> RuntimeRegistry {
    build_registry(
        bootstrap(BOOTSTRAP),
        definition(version, alias, upstream_model),
    )
    .expect("test registry must be valid")
}
