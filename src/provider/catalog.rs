//! Fixed researched provider entries (`docs/references/providers/`).
//!
//! These definitions pin origin, relative entries and auth scheme only. Model
//! capability is not claimed here; per-model facts live in topology bindings and
//! observed evidence.

use crate::provider::{
    auth::AuthScheme,
    definition::{EndpointPath, ProviderDefinition, ProviderId, TrustedOrigin},
};

/// DeepSeek OpenAI-compatible entries (`docs/references/providers/deepseek-api.md`).
pub fn deepseek() -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("deepseek").expect("static identity"),
        origin: TrustedOrigin::parse("https://api.deepseek.com").expect("static origin"),
        chat_completions: EndpointPath::new("/chat/completions").expect("static path"),
        responses: EndpointPath::new("/responses").expect("static path"),
        auth: AuthScheme::Bearer,
    }
}

/// Xiaomi MiMo entries (`docs/references/providers/xiaomi-api.md`). The official
/// docs accept `api-key:` or `Authorization: Bearer`; OpenBridge binds Bearer.
pub fn xiaomi() -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("xiaomi").expect("static identity"),
        origin: TrustedOrigin::parse("https://api.xiaomimimo.com").expect("static origin"),
        chat_completions: EndpointPath::new("/v1/chat/completions").expect("static path"),
        responses: EndpointPath::new("/v1/responses").expect("static path"),
        auth: AuthScheme::Bearer,
    }
}

/// OpenRouter aggregator entries (`docs/references/providers/openrouter-api.md`).
pub fn openrouter() -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("openrouter").expect("static identity"),
        origin: TrustedOrigin::parse("https://openrouter.ai").expect("static origin"),
        chat_completions: EndpointPath::new("/api/v1/chat/completions").expect("static path"),
        responses: EndpointPath::new("/api/v1/responses").expect("static path"),
        auth: AuthScheme::Bearer,
    }
}

pub fn all() -> Vec<ProviderDefinition> {
    vec![deepseek(), xiaomi(), openrouter()]
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
        assert_eq!(deepseek.responses.as_str(), "/responses");
        assert_eq!(deepseek.auth, AuthScheme::Bearer);

        let xiaomi = xiaomi();
        assert_eq!(xiaomi.id.as_str(), "xiaomi");
        assert_eq!(xiaomi.origin.as_str(), "https://api.xiaomimimo.com");
        assert_eq!(xiaomi.chat_completions.as_str(), "/v1/chat/completions");
        assert_eq!(xiaomi.responses.as_str(), "/v1/responses");
        assert_eq!(xiaomi.auth, AuthScheme::Bearer);

        assert_eq!(all().len(), 3);
    }
}
