//! OpenAI adapter 的静态能力与允许配置边界。

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{CredentialKind, ProviderContract, ProviderKind},
};

/// OpenAI adapter 的静态能力与允许的 endpoint/credential 范围。
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::OpenAi,
    ApiCapabilities {
        chat_completions: EndpointCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: true,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: true,
            previous_response_id: true,
            background: false,
        },
    },
    &["public-api"],
    &[CredentialKind::ApiKey],
);
