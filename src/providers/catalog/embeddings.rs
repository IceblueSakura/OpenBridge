//! Registers the checked-in Embeddings Public Models and Native Routes.
//!
//! Embeddings uses a separate downstream operation and has no Bridge or fallback candidates. Each
//! model therefore owns one direct Native Route, isolated from generation surface expansion.

use crate::{
    core::OperationKind,
    registry::{ModelLifecycle, PublicModelConfig, ReasoningLevelPolicy, RouteConfig, RouteMode},
};

use super::routing::CompiledPublicModel;

/// Builds all checked-in Embeddings Native Routes and their Public Models.
pub(super) fn compiled_registrations() -> Vec<CompiledPublicModel> {
    vec![
        compiled_registration(
            "text-embedding-3-small-openai-embeddings",
            "openai-text-embedding-3-small",
            "text-embedding-3-small",
            "OpenAI text embedding model with a fixed Native execution path.",
        ),
        compiled_registration(
            "qwen3-7-text-embedding-bailian-embeddings",
            "bailian-qwen3-7-text-embedding",
            "qwen3.7-text-embedding",
            "Qwen3.7 text embedding model with a fixed Native execution path.",
        ),
    ]
}

/// Builds one fixed Embeddings Native Route and its downstream Public Model.
fn compiled_registration(
    route_id: &str,
    upstream_target: &str,
    public_model_id: &str,
    description: &str,
) -> CompiledPublicModel {
    // Bind the downstream operation directly to the dedicated trusted Target and API.
    let route = RouteConfig {
        id: route_id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_operation: OperationKind::EmbeddingsCreate,
        downstream_operation: OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    };

    // Publish exactly that Route without adding Bridge or fallback candidates.
    let public_model = PublicModelConfig {
        id: public_model_id.to_owned(),
        created: 1_785_715_200,
        display_name: public_model_id.to_owned(),
        description: Some(description.to_owned()),
        lifecycle: ModelLifecycle::active(),
        reasoning_level_policy: ReasoningLevelPolicy::Strict,
        routes: vec![route.id.clone()],
    };
    CompiledPublicModel {
        routes: vec![route],
        public_model,
    }
}
