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
    CanonicalModelTask, CanonicalTaskKind, CredentialPoolConfig, EmbeddingModelProfile,
    GenerationModelProfile, IgnorableGenerationParameter, ImageGenerationModelProfile,
    InputModality, ModelConfig, ModelContextLength, ModelLifecycle, ModelLifecycleStatus,
    NonStreamingConversion, OutputModality, ProviderInstanceConfig, PublicModelConfig,
    ReasoningLevel, ReasoningLevelMapping, ReasoningLevelPolicy, ReasoningLevels, ReasoningProfile,
    ReasoningSupport, RegistryConfig, RouteConfig, RouteMode, SpeechRecognitionModelProfile,
    SpeechSynthesisModelProfile, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey,
    UpstreamApiModelRules, UpstreamStreamingPolicy, UpstreamTargetConfig, VoiceCloneModelProfile,
    VoiceDesignModelProfile,
};
pub use error::RegistryError;
pub use public_model::{
    AudioInputInterfaceCapabilities, AudioInputLimits, AudioInterfaceCapabilities,
    AudioOutputInterfaceCapabilities, ContextWindow, EmbeddingDimensionCapabilities,
    EmbeddingEncodingCapabilities, EmbeddingInterfaceCapabilities, EmbeddingLimits,
    ImageDetailCapabilities, ImageInputInterfaceCapabilities, ImageInputLimits,
    ImagesInterfaceCapabilities, InterfaceReasoningCapabilities, ModelCapabilities,
    ModelInterfaceCapabilities, ModelInterfaces, ModelModalities, ModelReasoningCapabilities,
    ModelTask, MultimodalInputCapabilities, MultimodalOutputCapabilities, PublicModel,
    PublicModelInfo, ReasoningOutputMode, StandardModel, StateCapabilities, StructuredOutputMode,
    SupportState, ToolCapabilities, ToolChoiceMode, ToolType,
};
pub(crate) use public_model::{FileInputSource, ModelExecutionInterface, OperationResponseBudget};
pub use runtime::{
    CredentialPoolBinding, ModelInfo, ProviderInstance, RegistryVersion, Route, RuntimeRegistry,
    UpstreamApi, UpstreamTarget,
};
