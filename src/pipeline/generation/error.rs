//! Generation request analysis and Route-planning errors.

use thiserror::Error;

/// Closed low-cardinality reason for one Generation fixed-interface capability rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationCapabilityReason {
    Protocol,
    Streaming,
    NonStreaming,
    Tools,
    ToolChoice,
    ParallelToolCalls,
    StrictToolSchema,
    StructuredOutput,
    PreviousResponse,
    Background,
    ResponseInclude,
    ImageInput,
    FileInput,
    AudioInput,
    AudioOutput,
    OutputLimit,
    Reasoning,
    ReasoningLevel,
    OrdinaryParameter,
    BridgeRepresentation,
}

/// Planning error returned when a request fails Public Model preflight or cannot bind to a configured Route.
#[derive(Debug, Error)]
pub enum RequestPlanningError {
    /// The request body is not a JSON object.
    #[error("request body must be a JSON object")]
    InvalidJson,
    /// The request lacks a non-empty Public Model.
    #[error("request body must contain a non-empty model")]
    MissingModel,
    /// Responses instructions are explicit but not one non-blank string.
    #[error("instructions must be a non-blank string")]
    InvalidInstructions,
    /// The Chat messages envelope cannot support deterministic instruction selection.
    #[error("messages must be a non-empty valid Chat message array")]
    InvalidMessages,
    /// The gateway accepts only stateless generation requests.
    #[error("store must be false when present")]
    InvalidStore,
    /// The request contains a top-level field outside the selected source protocol catalog.
    #[error("request contains unknown top-level parameter {0}")]
    UnknownParameter(String),
    /// A known standard top-level field has an invalid shape or value.
    #[error("request contains invalid parameter {0}")]
    InvalidParameter(&'static str),
    /// The requested Public Model is not registered.
    #[error("requested model is not configured")]
    UnknownModel,
    /// The Public Model has no statically executable Route.
    #[error("configured model has no executable route")]
    NoRoute,
    /// The Public Model's fixed interface cannot satisfy one standard Generation field.
    #[error("selected model does not support the requested capability in {param}")]
    UnsupportedModelCapability {
        /// Standard top-level request field that owns the rejected requirement.
        param: &'static str,
        /// Internal closed classification; never serialized to downstream clients.
        reason: GenerationCapabilityReason,
    },
    /// The request uses a named but unimplemented reserved capability.
    #[error("requested parameter {param} is reserved but not implemented")]
    UnimplementedCapabilities {
        /// Standard top-level owner of the reserved request feature.
        param: &'static str,
    },

    /// The request provides conflicting reasoning configuration sources or shapes.
    #[error("request contains conflicting reasoning configuration in {param}")]
    InvalidReasoningConfiguration { param: &'static str },
    /// Chat stream_options is outside the supported usage-tail and no-op request shapes.
    #[error("request contains invalid stream_options")]
    InvalidStreamOptions,
    /// A multimodal content part is malformed or appears outside its protocol-defined position.
    #[error("request contains invalid multimodal input")]
    InvalidMultimodalInput,
    /// Internal checked-size failure that analysis owners must locate before returning.
    #[error("request multimodal input exceeds a checked local limit")]
    MultimodalInputLimitExceeded,
}

impl RequestPlanningError {
    /// Creates one field-located fixed-interface capability rejection.
    pub(in crate::pipeline) const fn unsupported(
        param: &'static str,
        reason: GenerationCapabilityReason,
    ) -> Self {
        Self::UnsupportedModelCapability { param, reason }
    }

    /// Converts one internal multimodal analysis failure into its standard top-level owner.
    pub(in crate::pipeline) fn locate_multimodal(
        self,
        param: &'static str,
        reason: GenerationCapabilityReason,
    ) -> Self {
        match self {
            Self::InvalidMultimodalInput => Self::InvalidParameter(param),
            Self::MultimodalInputLimitExceeded => {
                Self::UnsupportedModelCapability { param, reason }
            }
            other => other,
        }
    }
}
