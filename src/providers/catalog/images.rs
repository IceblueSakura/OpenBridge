//! Registers the checked-in Images Generations Public Models and Native Routes.
//!
//! Images Generations uses a separate downstream operation and has no Bridge or fallback
//! candidates. Each model therefore owns one direct Native Route, isolated from generation
//! surface expansion and Embeddings registration.

use crate::{
    core::OperationKind,
    registry::{ModelLifecycle, PublicModelConfig, ReasoningLevelPolicy, RouteConfig},
};

/// Builds all checked-in Images Generations Native Routes and their Public Models.
pub(super) fn compiled_registrations() -> Vec<PublicModelConfig> {
    vec![
        compiled_registration(
            "bailian/qwen-image-3-0",
            "qwen-image-3.0",
            "Qwen Image 3.0 image generation model with a fixed Native execution path.",
        ),
        compiled_registration(
            "bailian/qwen-image-3-0-pro",
            "qwen-image-3.0-pro",
            "Qwen Image 3.0 Pro image generation and editing model with a fixed Native execution path.",
        ),
    ]
}

/// Builds one fixed Images Generations Native Route and its downstream Public Model.
fn compiled_registration(
    upstream_target: &str,
    public_model_id: &str,
    description: &str,
) -> PublicModelConfig {
    // Bind the downstream operation directly to the dedicated trusted Target and API.
    let route = RouteConfig {
        upstream_target: upstream_target.to_owned(),
        upstream_operation: OperationKind::ImagesGenerations,
        downstream_operation: OperationKind::ImagesGenerations,
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
