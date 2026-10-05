//! Explicit canonical declarations. Upstream aliases and public routes never create models.
use crate::{
    semantic::task::generation::GenerationSemanticContract,
    topology::{CanonicalModel, ModelId, TaskKind},
};
pub struct ModelDefinition {
    pub id: &'static str,
    pub images: bool,
    pub files: bool,
}
pub const MODELS: &[ModelDefinition] = &[
    ModelDefinition {
        id: "gpt-6.1-sol",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "grok-4.7",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "deepseek-flash",
        images: true,
        files: false,
    },
    ModelDefinition {
        id: "mimo-v2.6-pro",
        images: true,
        files: false,
    },
    ModelDefinition {
        id: "mimo-v2.6-flash",
        images: true,
        files: false,
    },
    // Source: https://openrouter.ai/api/v1/models/openai/gpt-6-luna/endpoints
    ModelDefinition {
        id: "gpt-6-luna",
        images: true,
        files: true,
    },
    ModelDefinition {
        id: "longcat-2.5-preview",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "nemotron-3-super",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "qwen3.8-max",
        images: true,
        files: false,
    },
    ModelDefinition {
        id: "qwen3.8-flash",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "minicpm5-1b",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "minicpm5-2b",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "minicpm-v-4.6",
        images: true,
        files: false,
    },
    ModelDefinition {
        id: "hy4-preview",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "kimi-k3",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "glm-5.3",
        images: false,
        files: false,
    },
    ModelDefinition {
        id: "glm-5.3-flash",
        images: true,
        files: false,
    },
];
pub fn contract(id: &str) -> GenerationSemanticContract {
    let definition = MODELS
        .iter()
        .find(|m| m.id == id)
        .expect("explicit canonical reference");
    GenerationSemanticContract {
        image_input: definition.images,
        file_input: definition.files,
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
