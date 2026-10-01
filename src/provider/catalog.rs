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
        chat_completions: EndpointPath::new("/chat/completions").expect("static path"),
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
        chat_completions: EndpointPath::new("/v1/chat/completions").expect("static path"),
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
        chat_completions: EndpointPath::new("/api/v1/chat/completions").expect("static path"),
        responses: Some(EndpointPath::new("/api/v1/responses").expect("static path")),
        auth: AuthScheme::Bearer,
    }
}

fn chat_provider(id: &str, origin: &str, path: &str) -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new(id).expect("static identity"),
        origin: TrustedOrigin::parse(origin).expect("static origin"),
        chat_completions: EndpointPath::new(path).expect("static path"),
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

pub fn all() -> Vec<ProviderDefinition> {
    vec![
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_pins_researched_origins_paths_and_auth() {
        let deepseek = deepseek();
        assert_eq!(deepseek.id.as_str(), "deepseek");
        assert_eq!(deepseek.origin.as_str(), "https://api.deepseek.com");
        assert_eq!(deepseek.chat_completions.as_str(), "/chat/completions");
        assert_eq!(deepseek.responses.unwrap().as_str(), "/responses");
        assert_eq!(deepseek.auth, AuthScheme::Bearer);

        let xiaomi = xiaomi();
        assert_eq!(xiaomi.id.as_str(), "xiaomi");
        assert_eq!(xiaomi.origin.as_str(), "https://api.xiaomimimo.com");
        assert_eq!(xiaomi.chat_completions.as_str(), "/v1/chat/completions");
        assert_eq!(xiaomi.responses.unwrap().as_str(), "/v1/responses");
        assert_eq!(xiaomi.auth, AuthScheme::Bearer);
    }
}
