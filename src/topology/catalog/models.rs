//! Explicit canonical declarations. Upstream aliases and public routes never create models.
use crate::{
    semantic::task::generation::GenerationSemanticContract,
    topology::{CanonicalModel, ModelId, TaskKind},
};
pub struct ModelDefinition {
    pub id: &'static str,
    pub images: bool,
}
pub const MODELS: &[ModelDefinition] = &[
    ModelDefinition {
        id: "deepseek-flash",
        images: true,
    },
    ModelDefinition {
        id: "mimo-v2.6-pro",
        images: true,
    },
    ModelDefinition {
        id: "mimo-v2.6-flash",
        images: true,
    },
    ModelDefinition {
        id: "gpt-6-luna",
        images: true,
    },
    ModelDefinition {
        id: "longcat-2.5-preview",
        images: false,
    },
    ModelDefinition {
        id: "nemotron-3-super",
        images: false,
    },
    ModelDefinition {
        id: "qwen3.8-max",
        images: true,
    },
    ModelDefinition {
        id: "qwen3.8-flash",
        images: false,
    },
    ModelDefinition {
        id: "kimi-k3",
        images: false,
    },
    ModelDefinition {
        id: "glm-5.3",
        images: false,
    },
    ModelDefinition {
        id: "glm-5.3-flash",
        images: true,
    },
];
pub fn contract(id: &str) -> GenerationSemanticContract {
    let definition = MODELS
        .iter()
        .find(|m| m.id == id)
        .expect("explicit canonical reference");
    GenerationSemanticContract {
        image_input: definition.images,
        ..GenerationSemanticContract::text_images()
    }
}
pub fn canonical_models() -> Vec<CanonicalModel> {
    MODELS
        .iter()
        .map(|m| CanonicalModel {
            id: ModelId::new(m.id).expect("static id"),
            task: TaskKind::Generation,
            contract: contract(m.id),
        })
        .collect()
}
