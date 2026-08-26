//! Registers the checked-in Embeddings Public Models and Native Routes.
//!
//! Embeddings uses a separate downstream operation and has no Bridge or fallback candidates. Each
//! model therefore owns one direct Native Route, isolated from generation surface expansion.

use crate::{
    core::OperationKind,
    registry::{ModelLifecycle, PublicModelConfig, ReasoningLevelPolicy, RouteConfig},
};

/// Builds all checked-in Embeddings Native Routes and their Public Models.
pub(super) fn compiled_registrations() -> Vec<PublicModelConfig> {
    vec![
        compiled_registration(
            "openai-text-embedding-3-small",
            "text-embedding-3-small",
            "OpenAI text embedding model with a fixed Native execution path.",
        ),
        compiled_registration(
            "bailian-qwen3-7-text-embedding",
            "qwen3.7-text-embedding",
            "Qwen3.7 text embedding model with a fixed Native execution path.",
        ),
        compiled_registration(
            "nvidia-nemotron-3-embed-1b",
            "nemotron-3-embed-1b",
            "NVIDIA Nemotron 3 Embed 1B text embedding model with a fixed Native execution path.",
        ),
    ]
}

/// Builds one fixed Embeddings Native Route and its downstream Public Model.
fn compiled_registration(
    upstream_target: &str,
    public_model_id: &str,
    description: &str,
) -> PublicModelConfig {
    // Bind the downstream operation directly to the dedicated trusted Target and API.
    let route = RouteConfig {
        upstream_target: upstream_target.to_owned(),
        upstream_operation: OperationKind::EmbeddingsCreate,
        downstream_operation: OperationKind::EmbeddingsCreate,
    };

    // Publish exactly that Route without adding Bridge or fallback candidates.
    PublicModelConfig {
        id: public_model_id.to_owned(),
        created: 1_785_715_200,
        display_name: public_model_id.to_owned(),
        description: Some(description.to_owned()),
        lifecycle: ModelLifecycle::active(),
        reasoning_level_policy: ReasoningLevelPolicy::Strict,
        routes: vec![route],
    }
}
