//! Facade for the compile-time Public Model route catalog.
//!
//! Concrete generation registrations, route expansion rules, and the independent Embeddings
//! registration are kept in focused child modules. This facade preserves one catalog entry point
//! so the outer registry assembly does not depend on their internal layout.

use crate::registry::PublicModelConfig;

use super::{embeddings, images, public_models, route_compiler};

/// Returns all Public Models with their ordered typed Routes compiled into the binary.
pub(super) fn compiled_public_models() -> Vec<PublicModelConfig> {
    // Compile generation registrations in their explicit model and Provider order.
    let mut public_models =
        route_compiler::compile_generation_routing(public_models::generation_registrations());

    // Append each independent Embeddings registration after all generation Public Models.
    for embedding in embeddings::compiled_registrations() {
        public_models.push(embedding);
    }

    // Append each independent Images Generations registration after all Embeddings Public Models.
    for images in images::compiled_registrations() {
        public_models.push(images);
    }
    public_models
}
