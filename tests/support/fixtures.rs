//! Shared configuration, credential, and RoutePlan fixtures for integration tests.

use std::time::Duration;

use openbridge::{
    config::{BootstrapConfig, parse_bootstrap_config},
    core::{
        ApiCapabilities, ApiProtocol, ChatCompletionsCapabilities, ReasoningOutput,
        ResponsesCapabilities,
    },
    credential::{CredentialMetadata, CredentialSource, CredentialStore},
    identity::{UserConfiguration, UserRegistry},
    pipeline::{RequestPlanningError, RoutePlan, analyze_request, plan_request},
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialPoolConfig, ModelConfig, ModelContextLength, ModelLifecycle, PublicModelConfig,
        ReasoningSupport, RegistryConfig, RouteConfig, RouteMode, RuntimeRegistry, StateAffinity,
        TransportKind, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTargetConfig, build_registry,
    },
};

use std::sync::Arc;

pub const BOOTSTRAP: &str = r#"
schema_version = 2
listen = "127.0.0.1:8080"
users_file = "config/users.toml"
upstream_credentials_file = "config/upstream-credentials.toml"
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
    users_and_credential_pool(api_key, registry, &[upstream_secret])
}

pub fn users_and_credential_pool(
    api_key: &str,
    registry: &RuntimeRegistry,
    upstream_secrets: &[&str],
) -> (Arc<UserRegistry>, Arc<CredentialStore>) {
    // Parse downstream users and obtain the same credential builder.
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

    // Inject one synthetic secret into every pool in the test registry.
    for pool_id in registry.credential_pool_ids() {
        let pool = registry.credential_pool(pool_id).unwrap();
        for (index, upstream_secret) in upstream_secrets.iter().enumerate() {
            credentials
                .insert_upstream_member(
                    pool.provider(),
                    pool.id(),
                    format!("{}#{}", pool.id(), index + 1),
                    secrecy::SecretString::from((*upstream_secret).to_owned()),
                    CredentialMetadata::upstream(pool.kind(), CredentialSource::Programmatic),
                )
                .expect("test upstream credential member must be unique");
        }
    }
    (Arc::new(users), Arc::new(credentials.build()))
}

pub fn capabilities() -> ApiCapabilities {
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            audio_input: false,
            file_input: false,
            audio_output: false,
            predicted_outputs: false,
            web_search: false,
            prompt_caching: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
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
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: false,
            conversation: false,
            prompt_templates: false,
            prompt_caching: false,
            context_management: false,
            include: &[],
            moderation: false,
            logprobs: false,
        },
        embeddings: Default::default(),
    }
}

pub fn definition(version: &str, alias: &str, upstream_model: &str) -> RegistryConfig {
    RegistryConfig {
        version: version.to_owned(),
        models: vec![ModelConfig {
            id: "openai/test-model".to_owned(),
            name: "Test model".to_owned(),
            description: Some("Model used by integration tests.".to_owned()),
            context_length: ModelContextLength::new(Some(128_000), None, Some(8_192)),
            mode: None,
            input_modalities: None,
            output_modalities: None,
            tokenizer: None,
            knowledge_cutoff: None,
            supported_parameters: Vec::new(),
            reasoning: ReasoningSupport::Unknown,
            reasoning_levels: Vec::new(),
        }],
        credential_pools: vec![CredentialPoolConfig {
            id: "openai-primary".to_owned(),
            provider: ProviderKind::OpenAi,
            kind: CredentialKind::ApiKey,
        }],
        upstream_targets: vec![UpstreamTargetConfig {
            id: "openai-main".to_owned(),
            provider: ProviderKind::OpenAi,
            model: "openai/test-model".to_owned(),
            base_url: "https://api.openai.com".to_owned(),
            credential_pool: "openai-primary".to_owned(),
            quota_scope: None,
            fault_domain: None,
            request_timeout: Duration::from_secs(120),
            enabled: true,
            upstream_apis: vec![
                UpstreamApiConfig {
                    id: "chat".to_owned(),
                    operation: ApiProtocol::ChatCompletions.operation(),
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
                    operation: ApiProtocol::Responses.operation(),
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
                downstream_operation: ApiProtocol::ChatCompletions.operation(),
                mode: RouteMode::Native,
            },
            RouteConfig {
                id: "public-responses".to_owned(),
                upstream_target: "openai-main".to_owned(),
                upstream_api: "responses".to_owned(),
                downstream_operation: ApiProtocol::Responses.operation(),
                mode: RouteMode::Native,
            },
        ],
        public_models: vec![PublicModelConfig {
            id: alias.to_owned(),
            created: 1_785_715_200,
            display_name: "Test public model".to_owned(),
            description: Some("Public model used by integration tests.".to_owned()),
            lifecycle: ModelLifecycle::active(),
            routes: vec!["public-chat".to_owned(), "public-responses".to_owned()],
        }],
    }
}

pub fn prepare(
    registry: &RuntimeRegistry,
    protocol: ApiProtocol,
    body: bytes::Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    let requirements = analyze_request(protocol, &body)?;
    plan_request(registry, &requirements, body)
}

pub fn registry(version: &str, alias: &str, upstream_model: &str) -> RuntimeRegistry {
    build_registry(
        bootstrap(BOOTSTRAP),
        definition(version, alias, upstream_model),
    )
    .expect("test registry must be valid")
}
