//! Registers the OpenAI Upstream Target and Native Upstream APIs.

use std::time::Duration;

use crate::{
    core::{ApiCapabilities, ChatCompletionsCapabilities, ReasoningOutput, ResponsesCapabilities},
    models::gpt,
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::UpstreamTargetConfig,
};

/// Builds the OpenAI upstream targets built into this compiled version.
pub fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "openai-main".to_owned(),
        provider: ProviderKind::OpenAi,
        model: gpt::v5_6_sol::ID.to_owned(),
        base_url: "https://api.openai.com".to_owned(),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis(
            "gpt-5.6-sol",
            "public-api",
            conservative_openai_capabilities(),
        ),
    }]
}

/// Returns conservative OpenAI capabilities that must be expanded only after an upstream probe.
pub const fn conservative_openai_capabilities() -> ApiCapabilities {
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
        embeddings: crate::core::EmbeddingsCapabilities::disabled(),
    }
}
