//! Package entry point for compile-time definitions and the request-path read-only registry.
//!
//! Submodules own static definitions, compilation errors, runtime entities, and startup compilation
//! logic; this file only declares modules and preserves the existing public API paths.

mod availability;
mod compiler;
mod definition;
mod error;
mod public_model;
mod runtime;
mod validation;

pub use availability::ConfigurationAvailabilityReport;
pub use compiler::{build_registry, build_registry_with_active_pools};
pub use definition::{
    CredentialPoolConfig, InputModality, ModelConfig, ModelContextLength, ModelLifecycle,
    ModelLifecycleStatus, ModelMode, NonStreamingConversion, OutputModality,
    ProviderInstanceConfig, PublicModelConfig, ReasoningLevel, ReasoningLevelMapping,
    ReasoningSupport, RegistryConfig, RouteConfig, RouteMode, StateAffinity,
    UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules, UpstreamStreamingPolicy,
    UpstreamTargetConfig,
};
pub use error::RegistryError;
pub(crate) use public_model::ModelExecutionInterface;
pub use public_model::{
    AudioInputInterfaceCapabilities, AudioInputLimits, AudioOutputInterfaceCapabilities,
    ContextWindow, EmbeddingDimensionCapabilities, EmbeddingEncodingCapabilities,
    EmbeddingInterfaceCapabilities, EmbeddingLimits, ImageDetailCapabilities,
    ImageInputInterfaceCapabilities, ImageInputLimits, InterfaceReasoningCapabilities,
    ModelCapabilities, ModelInterfaceCapabilities, ModelInterfaces, ModelModalities,
    ModelReasoningCapabilities, ModelTask, MultimodalInputCapabilities,
    MultimodalOutputCapabilities, PublicModel, PublicModelInfo, ReasoningOutputMode, StandardModel,
    StateCapabilities, StructuredOutputCapabilities, StructuredOutputMode, SupportState,
    ToolCapabilities, ToolChoiceMode, ToolType,
};
pub use runtime::{
    CredentialPoolBinding, ModelInfo, ProviderInstance, RegistryVersion, Route, RuntimeRegistry,
    UpstreamApi, UpstreamTarget,
};
