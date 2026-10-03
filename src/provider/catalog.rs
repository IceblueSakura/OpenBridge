//! Trusted origin, protocol path and authentication definitions, not a live inventory.
//!
//! Follow topology bindings and gateway bootstrap for public-model admission.
//! Official source links are lookup starting points; recheck them when changing a
//! binding. A registered entry does not prove account access or live compatibility.

use crate::provider::{
    auth::AuthScheme,
    definition::{EndpointPath, ProviderDefinition, ProviderId, TrustedOrigin},
};

/// OpenAI-compatible entries; source: <https://api-docs.deepseek.com/guides/responses_api/>.
pub fn deepseek() -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("deepseek").expect("static identity"),
        origin: TrustedOrigin::parse("https://api.deepseek.com").expect("static origin"),
        chat_completions: Some(EndpointPath::new("/chat/completions").expect("static path")),
        responses: Some(EndpointPath::new("/responses").expect("static path")),
        auth: AuthScheme::Bearer,
    }
}

/// MiMo binds Bearer rather than exposing authentication choice to requests.
/// Source: <https://mimo.mi.com/docs/zh-CN/api/chat/responses>.
pub fn xiaomi() -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("xiaomi").expect("static identity"),
        origin: TrustedOrigin::parse("https://api.xiaomimimo.com").expect("static origin"),
        chat_completions: Some(EndpointPath::new("/v1/chat/completions").expect("static path")),
        responses: Some(EndpointPath::new("/v1/responses").expect("static path")),
        auth: AuthScheme::Bearer,
    }
}

/// Go's model-specific entries are not interchangeable with the Zen balance API.
/// Hy4 is documented on Chat only; other Go models do not admit Responses here.
/// Source: <https://opencode.ai/docs/go/#endpoints>.
pub fn opencode_go() -> ProviderDefinition {
    chat_provider(
        "opencode-go",
        "https://opencode.ai",
        "/zen/go/v1/chat/completions",
    )
}

/// Aggregator entries; source: <https://openrouter.ai/docs/api/reference/responses/overview>.
pub fn openrouter() -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("openrouter").expect("static identity"),
        origin: TrustedOrigin::parse("https://openrouter.ai").expect("static origin"),
        chat_completions: Some(EndpointPath::new("/api/v1/chat/completions").expect("static path")),
        responses: Some(EndpointPath::new("/api/v1/responses").expect("static path")),
        auth: AuthScheme::Bearer,
    }
}

fn chat_provider(id: &str, origin: &str, path: &str) -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new(id).expect("static identity"),
        origin: TrustedOrigin::parse(origin).expect("static origin"),
        chat_completions: Some(EndpointPath::new(path).expect("static path")),
        responses: None,
        auth: AuthScheme::Bearer,
    }
}

/// Native Responses entry is explicit, not inferred from Chat compatibility.
/// Source: <https://longcat.chat/platform/docs/Codex.html>.
pub fn longcat() -> ProviderDefinition {
    ProviderDefinition {
        responses: Some(EndpointPath::new("/openai/v1/responses").expect("static path")),
        ..chat_provider(
            "longcat",
            "https://api.longcat.chat",
            "/openai/v1/chat/completions",
        )
    }
}
/// Hosted origin, not an arbitrary self-hosted NIM deployment.
/// Source: <https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html>.
pub fn nvidia() -> ProviderDefinition {
    ProviderDefinition {
        responses: Some(EndpointPath::new("/v1/responses").expect("static path")),
        ..chat_provider(
            "nvidia",
            "https://integrate.api.nvidia.com",
            "/v1/chat/completions",
        )
    }
}
/// Region-bound endpoint; credentials must belong to the selected region.
/// Sources: <https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions>,
/// <https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-responses>.
pub fn aliyun_dashscope_cn() -> ProviderDefinition {
    ProviderDefinition {
        responses: Some(EndpointPath::new("/compatible-mode/v1/responses").expect("static path")),
        ..chat_provider(
            "aliyun-dashscope-cn",
            "https://dashscope.aliyuncs.com",
            "/compatible-mode/v1/chat/completions",
        )
    }
}
/// Subscription credentials belong to this origin, never the metered endpoint.
/// Native Responses is declared by the official Codex configuration, not inferred
/// from the shared Chat wire. These entries do not bypass plan usage restrictions.
/// Sources: <https://help.aliyun.com/zh/model-studio/base-url>,
/// <https://help.aliyun.com/zh/model-studio/codex>.
pub fn aliyun_tokenplan_cn() -> ProviderDefinition {
    ProviderDefinition {
        responses: Some(EndpointPath::new("/compatible-mode/v1/responses").expect("static path")),
        ..chat_provider(
            "aliyun-tokenplan-cn",
            "https://token-plan.cn-beijing.maas.aliyuncs.com",
            "/compatible-mode/v1/chat/completions",
        )
    }
}
/// Source: <https://platform.kimi.com/docs/api/chat>.
pub fn kimi() -> ProviderDefinition {
    chat_provider("kimi", "https://api.moonshot.cn", "/v1/chat/completions")
}
/// The native Responses base path differs from Chat; never append to the Chat base.
/// Sources: <https://docs.bigmodel.cn/cn/guide/develop/openai/introduction>,
/// <https://docs.bigmodel.cn/cn/guide/develop/responses/introduction>.
pub fn zhipu() -> ProviderDefinition {
    ProviderDefinition {
        responses: Some(EndpointPath::new("/api/v1/responses").expect("static path")),
        ..chat_provider(
            "zhipu",
            "https://open.bigmodel.cn",
            "/api/paas/v4/chat/completions",
        )
    }
}

/// Product Responses backend, not the public SIWC or Platform API.
/// Source: https://github.com/openai/codex/tree/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs
pub fn codex() -> ProviderDefinition {
    ProviderDefinition {
        auth: AuthScheme::OAuthBearer("codex"),
        responses: Some(EndpointPath::new("/backend-api/codex/responses").expect("static path")),
        id: ProviderId::new("codex").expect("static identity"),
        origin: TrustedOrigin::parse("https://chatgpt.com").expect("static origin"),
        chat_completions: None,
    }
}
/// Public Responses OAuth route; not the Grok CLI subscription proxy.
/// Source: https://docs.x.ai/developers/model-capabilities/text/generate-text
pub fn grok() -> ProviderDefinition {
    ProviderDefinition {
        auth: AuthScheme::OAuthBearer("grok"),
        responses: Some(EndpointPath::new("/v1/responses").expect("static path")),
        id: ProviderId::new("grok").expect("static identity"),
        origin: TrustedOrigin::parse("https://api.x.ai").expect("static origin"),
        chat_completions: None,
    }
}
pub fn all() -> Vec<ProviderDefinition> {
    vec![
        codex(),
        grok(),
        deepseek(),
        xiaomi(),
        openrouter(),
        opencode_go(),
        longcat(),
        nvidia(),
        aliyun_dashscope_cn(),
        aliyun_tokenplan_cn(),
        kimi(),
        zhipu(),
    ]
}
