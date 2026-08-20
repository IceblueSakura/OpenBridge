//! Registers the OpenAI Upstream Target and Native Upstream APIs.

use std::time::Duration;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        ExecutableResponsesState, FunctionToolCapabilities, ResponsesAffinity, StorageSupport,
    },
    models::openai,
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::{
        CanonicalTaskKind, ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiKey, UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "openai";

const EMBEDDING_INPUT_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::String,
    EmbeddingInputForm::StringArray,
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const EMBEDDING_ENCODINGS: &[EmbeddingEncoding] =
    &[EmbeddingEncoding::Float, EmbeddingEncoding::Base64];
const LOCALLY_COUNTED_EMBEDDING_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const EMBEDDING_PARAMETERS: &[&str] = &["encoding_format", "user"];
const CONSERVATIVE_FUNCTION_TOOLS: FunctionToolCapabilities = FunctionToolCapabilities {
    choice_modes: ALL_TOOL_CHOICE_MODES,
    parallel_calls: false,
    strict_schema: false,
};

/// Builds the trusted OpenAI API deployment used by the checked-in targets.
pub fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::OpenAi,
        base_url: "https://api.openai.com".to_owned(),
    }
}

/// Builds the OpenAI upstream targets built into this compiled version.
pub fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        generation_target("openai-main", openai::gpt_5_6_sol::ID, "gpt-5.6-sol"),
        generation_target(
            "openai-gpt-5-6-terra",
            openai::gpt_5_6_terra::ID,
            "gpt-5.6-terra",
        ),
        generation_target(
            "openai-gpt-5-6-luna",
            openai::gpt_5_6_luna::ID,
            "gpt-5.6-luna",
        ),
        generation_target("openai-gpt-5-5", openai::gpt_5_5::ID, "gpt-5.5"),
        embedding_target(),
    ]
}

/// Builds one OpenAI generation target without adding request-time model selection.
fn generation_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
) -> UpstreamTargetConfig {
    // Resolve both operation profiles required by every checked-in OpenAI generation target.
    let capabilities = DEFINITION.contract().capabilities();
    let mut chat_capabilities = capabilities
        .operation(crate::core::OperationKind::ChatCompletions)
        .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
        .expect("OpenAI generation targets require Chat Completions capabilities")
        .to_executable(crate::core::ChatMediaProfile::default());
    let mut responses_capabilities = capabilities
        .operation(crate::core::OperationKind::Responses)
        .and_then(crate::core::ProviderOperationCapabilities::responses)
        .expect("OpenAI generation targets require Responses capabilities")
        .to_executable(
            ExecutableResponsesState::new(
                StorageSupport::Unsupported,
                ResponsesAffinity::TargetBound,
            ),
            crate::core::ResponsesMediaProfile::default(),
        );

    // Narrow unverified strict-tool, structured-output, and persistent-state features.
    chat_capabilities.function_tools = Some(CONSERVATIVE_FUNCTION_TOOLS);
    chat_capabilities.structured_outputs = None;
    chat_capabilities.store = false;
    responses_capabilities.function_tools = Some(CONSERVATIVE_FUNCTION_TOOLS);
    responses_capabilities.structured_outputs = None;

    // Bind the concrete operation profiles to the trusted OpenAI deployment.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::OpenAi.routing_model_id(canonical_model),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis(
            upstream_model,
            CanonicalTaskKind::Generation,
            chat_capabilities,
            Some(responses_capabilities),
        ),
    }
}

/// Builds the dedicated `text-embedding-3-small` target and its sole Native API.
fn embedding_target() -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: "openai-text-embedding-3-small".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: openai::text_embedding_3_small::ID.to_owned(),
        provider_model: ProviderKind::OpenAi.routing_model_id(openai::text_embedding_3_small::ID),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::EmbeddingsCreate,
                CanonicalTaskKind::Embedding,
            ),
            upstream_model: "text-embedding-3-small".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(text_embedding_3_small_capabilities()),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}

/// Returns the checked-in OpenAI `text-embedding-3-small` execution contract.
pub const fn text_embedding_3_small_capabilities() -> EmbeddingsCapabilities {
    EmbeddingsCapabilities {
        input_forms: EMBEDDING_INPUT_FORMS,
        default_encoding: EmbeddingEncoding::Float,
        allowed_encodings: Some(EMBEDDING_ENCODINGS),
        default_dimensions: 1_536,
        allowed_dimensions: None,
        max_inputs: 2_048,
        max_tokens_per_input: Some(8_192),
        max_total_tokens: Some(300_000),
        locally_counted_input_forms: LOCALLY_COUNTED_EMBEDDING_FORMS,
        supported_parameters: EMBEDDING_PARAMETERS,
    }
}
