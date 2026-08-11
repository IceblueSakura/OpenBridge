//! Shared configuration, credential, and RoutePlan fixtures for integration tests.

use std::{path::Path, sync::Arc, time::Duration};

use openbridge::{
    config::{BootstrapConfig, parse_bootstrap_config},
    core::{
        ALL_TOOL_CHOICE_MODES, ApiCapabilities, ApiProtocol, ExecutableResponsesState,
        FunctionToolCapabilities, ProviderChatCompletionsCapabilities,
        ProviderResponsesCapabilities, ProviderResponsesStateCeiling, ReasoningOutput,
        ResponsesAffinity, StorageSupport,
    },
    credential::{CredentialMetadata, CredentialSource, CredentialStore},
    identity::{UserConfiguration, UserRegistry},
    oauth2_credentials::OAuth2CredentialManager,
    pipeline::{RequestPlanningError, RoutePlan, analyze_request, plan_request},
    provider::{CredentialKind, ProviderKind},
    registry::{
        CanonicalModelTask, CredentialPoolConfig, GenerationModelProfile, ModelConfig,
        ModelContextLength, ModelLifecycle, ProviderInstanceConfig, PublicModelConfig,
        ReasoningProfile, RegistryConfig, RouteConfig, RouteMode, RuntimeRegistry,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules, UpstreamTargetConfig,
        build_registry,
    },
    upstream_credentials::UpstreamCredentialConfiguration,
};

pub const BOOTSTRAP: &str = r#"
schema_version = 2
listen = "127.0.0.1:8080"
users_file = "config/users.toml"
upstream_credentials_file = "config/upstream-credentials.toml"
default_instructions = "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
max_request_body_bytes = 1048576
max_json_response_body_bytes = 16777216
max_replay_body_bytes = 262144
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

/// Returns the generation payload owned by a synthetic canonical Model fixture.
pub fn generation_profile_mut(model: &mut ModelConfig) -> &mut GenerationModelProfile {
    match &mut model.task {
        CanonicalModelTask::Generation(profile) => profile,
        _ => panic!("the shared synthetic Model must remain a generation task"),
    }
}

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

    // Inject synthetic API keys only into pools required by enabled data-plane targets.
    for pool_id in registry.credential_pool_ids() {
        let pool = registry.credential_pool(pool_id).unwrap();
        if pool.kind() != CredentialKind::ApiKey {
            continue;
        }
        let required = registry.upstream_target_ids().any(|target_id| {
            let target = registry.upstream_target(target_id).unwrap();
            target.enabled() && target.credential_pool_id() == pool.id()
        });
        if !required {
            continue;
        }
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

pub fn users_and_oauth_credentials(
    api_key: &str,
    registry: &RuntimeRegistry,
    auth_json_file: &Path,
) -> (
    Arc<UserRegistry>,
    Arc<CredentialStore>,
    Arc<OAuth2CredentialManager>,
) {
    // Parse one downstream user and retain its shared credential builder.
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

    // Load one synthetic OpenBridge-owned auth file through the production configuration boundary.
    let locator = auth_json_file.to_string_lossy().replace('\\', "/");
    let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
        r#"
schema_version = 1

[[credential_pools]]
id = "chatgpt-codex"
auth_json_file = "{locator}"
"#
    ))
    .expect("test upstream OAuth2 configuration must be valid");
    let oauth2_credentials = configuration
        .load_into_for(&mut credentials, registry, ["chatgpt-codex"])
        .expect("synthetic ChatGPT OAuth2 bundle must load");

    // Freeze downstream credentials while retaining OAuth2 rotation in its guarded manager.
    (
        Arc::new(users),
        Arc::new(credentials.build()),
        Arc::new(oauth2_credentials),
    )
}

pub fn capabilities() -> ApiCapabilities {
    ApiCapabilities {
        chat_completions: Some(ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: false,
                strict_schema: false,
            }),
            image_input: None,
            structured_outputs: None,
            store: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            audio: None,
            file_input: false,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        }),
        responses: Some(ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: false,
                strict_schema: false,
            }),
            image_input: None,
            structured_outputs: None,
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: false,
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: false,
            context_management: false,
            include: &[],
            moderation: false,
            logprobs: false,
        }),
        embeddings: None,
    }
}

pub fn definition(version: &str, alias: &str, upstream_model: &str) -> RegistryConfig {
    RegistryConfig {
        version: version.to_owned(),
        models: vec![ModelConfig {
            id: "openai/test-model".to_owned(),
            name: "Test model".to_owned(),
            description: Some("Model used by integration tests.".to_owned()),
            tokenizer: None,
            knowledge_cutoff: None,
            task: CanonicalModelTask::Generation(GenerationModelProfile {
                context_length: ModelContextLength::new(Some(128_000), None, Some(8_192)),
                input_modalities: None,
                output_modalities: None,
                supported_parameters: Vec::new(),
                reasoning: ReasoningProfile::Unknown,
            }),
        }],
        provider_instances: vec![ProviderInstanceConfig {
            id: "openai".to_owned(),
            kind: ProviderKind::OpenAi,
            base_url: "https://api.openai.com".to_owned(),
        }],
        credential_pools: vec![CredentialPoolConfig {
            id: "openai-primary".to_owned(),
            provider: ProviderKind::OpenAi,
            kind: CredentialKind::ApiKey,
        }],
        upstream_targets: vec![UpstreamTargetConfig {
            id: "openai-main".to_owned(),
            provider_instance: "openai".to_owned(),
            canonical_model: "openai/test-model".to_owned(),
            provider_model: "openai/test-model".to_owned(),
            credential_pool: "openai-primary".to_owned(),
            quota_scope: None,
            fault_domain: None,
            request_timeout: Duration::from_secs(120),
            enabled: true,
            upstream_apis: vec![
                UpstreamApiConfig {
                    upstream_model: upstream_model.to_owned(),
                    model_rules: UpstreamApiModelRules::default(),
                    capabilities: UpstreamApiCapabilities::ChatCompletions(
                        capabilities()
                            .chat_completions
                            .expect("the synthetic Provider must expose Chat Completions")
                            .to_executable(None),
                    ),
                    streaming_policy: openbridge::registry::UpstreamStreamingPolicy::Optional,
                },
                UpstreamApiConfig {
                    upstream_model: upstream_model.to_owned(),
                    model_rules: UpstreamApiModelRules::default(),
                    capabilities: UpstreamApiCapabilities::Responses(
                        capabilities()
                            .responses
                            .expect("the synthetic Provider must expose Responses")
                            .to_executable(ExecutableResponsesState::new(
                                StorageSupport::Unsupported,
                                ResponsesAffinity::TargetBound,
                            )),
                    ),
                    streaming_policy: openbridge::registry::UpstreamStreamingPolicy::Optional,
                },
            ],
        }],
        routes: vec![
            RouteConfig {
                id: "public-chat".to_owned(),
                upstream_target: "openai-main".to_owned(),
                upstream_operation: ApiProtocol::ChatCompletions.operation(),
                downstream_operation: ApiProtocol::ChatCompletions.operation(),
                mode: RouteMode::Native,
            },
            RouteConfig {
                id: "public-responses".to_owned(),
                upstream_target: "openai-main".to_owned(),
                upstream_operation: ApiProtocol::Responses.operation(),
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
            reasoning_level_policy: openbridge::registry::ReasoningLevelPolicy::Strict,
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
