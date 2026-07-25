#![allow(dead_code)]

use std::time::Duration;

use openbridge::{
    config::{BootstrapPolicy, load_bootstrap},
    core::{CapabilitySet, ProtocolCapabilities, ResponsesCapabilities},
    provider::{CredentialKind, ProviderKind},
    registry::{
        AliasDefinition, CredentialDefinition, DeploymentDefinition, ModelConstraints,
        ModelContextLength, ModelDefinition, ProviderDefinition, ReasoningSupport,
        RegistryDefinition, RegistrySnapshot, build_registry,
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
        models: vec![ModelDefinition {
            id: "openai/test-model".to_owned(),
            name: "Test model".to_owned(),
            description: Some("Model used by integration tests.".to_owned()),
            context_length: ModelContextLength::new(Some(128_000), Some(8_192)),
            supported_parameters: Vec::new(),
            reasoning: ReasoningSupport::Unknown,
            reasoning_levels: Vec::new(),
        }],
        providers: vec![ProviderDefinition {
            id: "openai".to_owned(),
            kind: ProviderKind::OpenAi,
            credential: CredentialDefinition {
                id: "openai-primary".to_owned(),
                kind: CredentialKind::ApiKey,
                environment_variable: "OPENAI_API_KEY".to_owned(),
            },
        }],
        deployments: vec![DeploymentDefinition {
            id: "openai-main".to_owned(),
            provider: "openai".to_owned(),
            model: "openai/test-model".to_owned(),
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: "public-api".to_owned(),
            base_url: "https://api.openai.com".to_owned(),
            request_timeout: Duration::from_secs(120),
            model_constraints: ModelConstraints::default(),
            capabilities: capabilities(),
        }],
        aliases: vec![AliasDefinition {
            name: alias.to_owned(),
            candidates: vec!["openai-main".to_owned()],
        }],
    }
}

pub fn snapshot(version: &str, alias: &str, upstream_model: &str) -> RegistrySnapshot {
    build_registry(
        bootstrap(BOOTSTRAP),
        definition(version, alias, upstream_model),
    )
    .expect("test registry must be valid")
}
