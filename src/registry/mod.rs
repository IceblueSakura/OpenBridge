//! Package entry point for compile-time definitions and the request-path read-only registry.
//!
//! Submodules own static definitions, compilation errors, runtime entities, and startup compilation
//! logic; this file only declares modules and preserves the existing public API paths.

mod compiler;
mod definition;
mod error;
mod public_model;
mod runtime;
mod validation;

pub use compiler::{build_registry, build_registry_with_active_pools};
pub use definition::{
    CredentialPoolConfig, InputModality, ModelConfig, ModelContextLength, ModelLifecycle,
    ModelLifecycleStatus, ModelMode, OutputModality, ProviderInstanceConfig, PublicModelConfig,
    ReasoningLevel, ReasoningLevelMapping, ReasoningSupport, RegistryConfig, RouteConfig,
    RouteMode, StateAffinity, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
    UpstreamTargetConfig,
};
pub use error::RegistryError;
pub(crate) use public_model::ModelExecutionInterface;
pub use public_model::{
    ContextWindow, EmbeddingDimensionCapabilities, EmbeddingEncodingCapabilities,
    EmbeddingInterfaceCapabilities, EmbeddingLimits, InterfaceReasoningCapabilities,
    ModelCapabilities, ModelInterfaceCapabilities, ModelInterfaces, ModelModalities,
    ModelReasoningCapabilities, ModelTask, PublicModel, PublicModelInfo, ReasoningOutputMode,
    StandardModel, StateCapabilities, StructuredOutputCapabilities, StructuredOutputMode,
    SupportState, ToolCapabilities, ToolChoiceMode, ToolType,
};
pub use runtime::{
    CredentialPoolBinding, ModelInfo, ProviderInstance, RegistryVersion, Route, RuntimeRegistry,
    UpstreamApi, UpstreamTarget,
};
