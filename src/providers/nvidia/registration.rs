//! Registers the fixed NVIDIA API Catalog deployment and approved model Targets.

use std::time::Duration;

use crate::{
    models::{minimax, nvidia},
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiKey, UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::{DEFINITION, media::IMAGE_INPUT};

const PROVIDER_INSTANCE_ID: &str = "nvidia";
const CREDENTIAL_POOL_ID: &str = "nvidia-primary";

/// Builds the trusted NVIDIA API Catalog deployment used by approved Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Nvidia,
        base_url: "https://integrate.api.nvidia.com/v1".to_owned(),
    }
}

/// Builds the fixed MiniMax M3 Chat target and Nemotron 3 Embed 1B target for NVIDIA API Catalog.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        chat_target(
            "nvidia-minimax-m3",
            minimax::minimax_m3::ID,
            "minimaxai/minimax-m3",
        ),
        embedding_target(),
    ]
}

/// Binds one canonical model to NVIDIA's trusted Chat endpoint and credential pool.
fn chat_target(id: &str, canonical_model: &str, upstream_model: &str) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::Nvidia.routing_model_id(canonical_model),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("nvidia-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::ChatCompletions,
                CanonicalTaskKind::Generation,
            ),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(
                DEFINITION
                    .contract()
                    .capabilities()
                    .operation(crate::core::OperationKind::ChatCompletions)
                    .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
                    .expect("NVIDIA targets require Chat Completions capabilities")
                    .to_executable(crate::core::ChatMediaProfile::new(
                        Some(IMAGE_INPUT),
                        None,
                        None,
                    )),
            ),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}

/// Binds Nemotron 3 Embed 1B to NVIDIA's trusted Embeddings endpoint and credential pool.
fn embedding_target() -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: "nvidia-nemotron-3-embed-1b".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: nvidia::nemotron_3_embed_1b::ID.to_owned(),
        provider_model: ProviderKind::Nvidia.routing_model_id(nvidia::nemotron_3_embed_1b::ID),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("nvidia-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::EmbeddingsCreate,
                CanonicalTaskKind::Embedding,
            ),
            upstream_model: "nvidia/nemotron-3-embed-1b".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(
                DEFINITION
                    .contract()
                    .capabilities()
                    .operation(crate::core::OperationKind::EmbeddingsCreate)
                    .and_then(crate::core::ProviderOperationCapabilities::embeddings)
                    .expect("NVIDIA embedding targets require Embeddings capabilities"),
            ),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}
