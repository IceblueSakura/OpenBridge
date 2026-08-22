//! Facade for the compile-time Public Model route catalog.
//!
//! Concrete generation registrations, route expansion rules, and the independent Embeddings
//! registration are kept in focused child modules. This facade preserves one catalog entry point
//! so the outer registry assembly does not depend on their internal layout.

use crate::registry::{PublicModelConfig, RouteConfig};

use super::{embeddings, images, public_models, route_compiler};

/// Aggregated Route and Public Model definitions used by the compiled catalog.
pub(super) struct CompiledRouting {
    pub(super) routes: Vec<RouteConfig>,
    pub(super) public_models: Vec<PublicModelConfig>,
}

/// One compiled Public Model and its complete ordered Route set.
pub(super) struct CompiledPublicModel {
    pub(super) routes: Vec<RouteConfig>,
    pub(super) public_model: PublicModelConfig,
}

/// Returns all Public Models and their Routes compiled into the binary.
pub(super) fn compiled_routing() -> CompiledRouting {
    // Compile generation registrations in their explicit model and Provider order.
    let mut routing =
        route_compiler::compile_generation_routing(public_models::generation_registrations());

    // Append each independent Embeddings registration after all generation Public Models.
    for embedding in embeddings::compiled_registrations() {
        routing.routes.extend(embedding.routes);
        routing.public_models.push(embedding.public_model);
    }

    // Append each independent Images Generations registration after all Embeddings Public Models.
    for images in images::compiled_registrations() {
        routing.routes.extend(images.routes);
        routing.public_models.push(images.public_model);
    }
    routing
}
