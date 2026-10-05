//! Explicit OpenRouter Speech binding; product admission never follows Chat activation.
use crate::{
    adapter::speech::Profile,
    lowering::speech::SpeechCapabilities,
    provider::{CredentialBindingId, CredentialKind, EndpointPath, catalog},
    semantic::value::AudioEncoding,
    topology::{
        EndpointId, EndpointTarget, ExecutionContract, ModelId, ModelMetadata, RouteId,
        speech::{ProviderEntry, SpeechEndpoint, SpeechRoute},
    },
};
pub struct SpeechBinding {
    pub model: &'static str,
    pub upstream: &'static str,
    pub credential: &'static str,
    pub voices: &'static [&'static str],
    pub released_at: u64,
    pub developer: &'static str,
}
// Operation: https://openrouter.ai/docs/api/api-reference/tts/create-speech.md
// Endpoint: https://openrouter.ai/api/v1/models/qwen/qwen-audio-3.0-tts-flash-20260723/endpoints
// Named voices: https://openrouter.ai/api/v1/models?output_modalities=speech
pub const SPEECH_BINDINGS: &[SpeechBinding] = &[SpeechBinding {
    model: "qwen-audio-3.0-tts-flash",
    upstream: "qwen/qwen-audio-3.0-tts-flash",
    credential: "openrouter-api-key",
    voices: &["loongjohn", "longanhuan_v3.6"],
    // Developer release announcement (2026-07-21), UTC midnight, not router listing time.
    // https://www.alibabacloud.com/blog/qwen-audio-3-0-tts-more-multilingual-easier-to-direct_603379
    released_at: 1_784_592_000,
    developer: "Alibaba",
}];
impl SpeechBinding {
    pub fn metadata(&self) -> ModelMetadata {
        ModelMetadata::new(self.released_at, self.developer).expect("static publication")
    }
    pub fn provider(&self) -> crate::provider::ProviderDefinition {
        catalog::openrouter()
    }
    pub fn operation(&self) -> ProviderEntry {
        ProviderEntry {
            provider: self.provider().id,
            path: EndpointPath::new("/api/v1/audio/speech").expect("static Speech path"),
        }
    }
    pub fn route(&self) -> SpeechRoute {
        let provider = self.provider();
        let canonical = ModelId::new(self.model).expect("static speech identity");
        SpeechRoute {
            id: RouteId::new("openrouter-qwen-speech").expect("static route"),
            model: canonical.clone(),
            canonical_model: canonical.clone(),
            endpoint: SpeechEndpoint {
                id: EndpointId::new("openrouter-qwen-speech").expect("static endpoint"),
                provider: provider.id,
                target: EndpointTarget {
                    origin: provider.origin,
                    path: self.operation().path,
                },
                upstream_model: self.upstream.into(),
                canonical_model: canonical,
                profile: Profile::OpenRouterMp3,
                capabilities: SpeechCapabilities {
                    voices: self.voices.iter().map(|v| (*v).into()).collect(),
                    formats: vec![AudioEncoding::Mp3],
                    instructions: false,
                    speed: false,
                },
                credential: CredentialBindingId::new(self.credential).expect("static binding"),
                execution: ExecutionContract {
                    streaming: false,
                    retry_before_commit: false,
                    request_body_limit: 64 << 10,
                    response_body_limit: 8 << 20,
                    credential_kind: CredentialKind::ApiKey,
                    timeout_ms: 120_000,
                },
            },
        }
    }
}
