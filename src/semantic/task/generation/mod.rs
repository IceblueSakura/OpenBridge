//! Generation semantic IR.
mod continuation;
mod dependency;
pub use dependency::{HistoryDependency, RequestDependencyProof};
mod contract;
pub use continuation::{CallReference, Continuation};
pub use contract::{GenerationFeature, GenerationSemanticContract};
mod event;
mod group;
pub use group::MessageGroup;
mod output;
mod pattern;
mod reasoning;
mod request;
mod requirements;
mod resource;
mod response;
mod schema;
mod text;
mod tool;
mod turn;
pub use turn::{
    ContinuationError, ResponseContinuation, ResponseId, ResponseRelation, ResultReadiness, TurnId,
};
mod validate;
pub use output::{OutputConstraint, TextOptions, Verbosity};
pub use reasoning::{
    EncryptedReasoning, ReasoningContent, ReasoningContext, ReasoningEffort, ReasoningItem,
    ReasoningMode, ReasoningPresence, ReasoningReplay, ReasoningRequest, ReasoningSummary,
};
pub use request::{
    ConfigurationUpdate, ContentPart, GenerationControls, GenerationError, GenerationRequest,
    GenerationSettings, Instruction, InstructionAuthority, Item, ItemId, Message, MessageRole,
    Part, PartId, Phase, Truncation,
};
pub use requirements::GenerationRequirements;
pub use resource::{
    ImageDetail, ImageFormat, MAX_IMAGE_DECODED_BYTES, MAX_IMAGE_URL_BYTES, Resource, ResourceKind,
    ResourceLocation,
};
pub use text::{
    Annotation, Logprob, RefusalContent, TextContent, TopLogprob, compatible_logprobs,
    validate_logprobs,
};
pub use tool::{
    CallContext, CallOrigin, CallerMode, CustomCall, CustomFormat, CustomTool, FunctionStrictness,
    FunctionTool, GrammarSyntax, ItemLifecycle, Program, ProgramOutput, StrictDefault, ToolCall,
    ToolChoice, ToolDefinition, ToolDispatch, ToolKind, ToolOutput, ToolReference, ToolResult,
};

pub use event::{
    EventError, ItemKind, PartKind, StreamEvent, StreamItem, StreamPart, StreamState,
    StreamTerminal, end_of_stream, materialize, reduce, snapshot_items,
};
pub use response::{
    GenerationResponse, IncompleteReason, Outcome, ResponseError, TerminalDetails, Usage,
};
pub use validate::{MAX_ITEMS, MAX_TEXT_BYTES, MAX_TOOLS, MAX_TOTAL_BYTES};
