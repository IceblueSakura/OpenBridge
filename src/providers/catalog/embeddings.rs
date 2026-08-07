//! Registers the dedicated Embeddings Public Model and Native Route.
//!
//! Embeddings uses a separate downstream operation and has no Bridge or fallback candidates, so
//! its checked-in registration remains isolated from generation surface expansion.

use crate::{
    core::OperationKind,
    registry::{ModelLifecycle, PublicModelConfig, RouteConfig, RouteMode},
};

use super::routing::CompiledPublicModel;

/// Builds the single checked-in Embeddings Native Route and its Public Model.
pub(super) fn compiled_registration() -> CompiledPublicModel {
    // Bind the downstream operation directly to the dedicated trusted Target and API.
    let route = RouteConfig {
        id: "text-embedding-3-small-openai-embeddings".to_owned(),
        upstream_target: "openai-text-embedding-3-small".to_owned(),
        upstream_operation: OperationKind::EmbeddingsCreate,
        downstream_operation: OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    };

    // Publish exactly that Route without adding Bridge or fallback candidates.
    let public_model = PublicModelConfig {
        id: "text-embedding-3-small".to_owned(),
        created: 1_785_715_200,
        display_name: "text-embedding-3-small".to_owned(),
        description: Some(
            "OpenAI text embedding model with a fixed Native execution path.".to_owned(),
        ),
        lifecycle: ModelLifecycle::active(),
        routes: vec![route.id.clone()],
    };
    CompiledPublicModel {
        routes: vec![route],
        public_model,
    }
}
