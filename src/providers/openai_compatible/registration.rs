//! Shared static Upstream API registration for native OpenAI-compatible operations.

use crate::{
    core::{ChatCompletionsCapabilities, OperationKind, ResponsesCapabilities},
    registry::{
        CanonicalTaskKind, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey,
        UpstreamApiModelRules,
    },
};

/// Builds Chat followed by an optional Responses HTTP JSON/SSE Upstream API.
pub(crate) fn native_upstream_apis(
    upstream_model: &str,
    task: CanonicalTaskKind,
    chat_capabilities: ChatCompletionsCapabilities,
    responses_capabilities: Option<ResponsesCapabilities>,
) -> Vec<UpstreamApiConfig> {
    // Build the required stateless Chat API as the first operation.
    let mut upstream_apis = vec![UpstreamApiConfig {
        key: UpstreamApiKey::new(OperationKind::ChatCompletions, task),
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];

    // Append the target-bound Responses API only when the target exposes that operation.
    if let Some(responses_capabilities) = responses_capabilities {
        upstream_apis.push(UpstreamApiConfig {
            key: UpstreamApiKey::new(OperationKind::Responses, task),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        });
    }
    upstream_apis
}
