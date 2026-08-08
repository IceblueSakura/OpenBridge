//! Registers the OpenAI Upstream Target and Native Upstream APIs.

use std::time::Duration;

use crate::{
    core::{
        ApiCapabilities, ChatCompletionsCapabilities, EmbeddingEncoding, EmbeddingInputForm,
        EmbeddingsCapabilities, ReasoningOutput, ResponsesCapabilities,
    },
    models::openai,
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::{
        ProviderInstanceConfig, StateAffinity, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

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
    vec![generation_target(), embedding_target()]
}

/// Builds the existing OpenAI generation target without adding request-time model selection.
fn generation_target() -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: "openai-main".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: openai::gpt_5_6_sol::ID.to_owned(),
        provider_model: ProviderKind::OpenAi.routing_model_id(openai::gpt_5_6_sol::ID),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis("gpt-5.6-sol", conservative_openai_capabilities()),
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
            upstream_model: "text-embedding-3-small".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(text_embedding_3_small_capabilities()),
            state_affinity: StateAffinity::Unbound,
        }],
    }
}

/// Returns the checked-in OpenAI `text-embedding-3-small` execution contract.
pub const fn text_embedding_3_small_capabilities() -> EmbeddingsCapabilities {
    EmbeddingsCapabilities {
        enabled: true,
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

/// Returns conservative OpenAI capabilities that must be expanded only after an upstream probe.
pub const fn conservative_openai_capabilities() -> ApiCapabilities {
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: None,
            structured_outputs: false,
            store: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            audio_input: false,
            audio: None,
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
            image_input: None,
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
        embeddings: crate::core::EmbeddingsCapabilities::disabled(),
    }
}
