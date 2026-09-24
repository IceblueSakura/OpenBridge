//! Generation semantic IR.
mod event;
mod output;
mod reasoning;
mod request;
mod requirements;
mod resource;
mod response;
mod schema;
mod text;
mod tool;
mod validate;
pub use output::{OutputConstraint, TextOptions, Verbosity};
pub use reasoning::{
    EncryptedReasoning, ReasoningContent, ReasoningContext, ReasoningEffort, ReasoningItem,
    ReasoningMode, ReasoningPresence, ReasoningReplay, ReasoningRequest, ReasoningSummary,
};
pub use request::{
    ContentPart, GenerationControls, GenerationError, GenerationRequest, GenerationSettings,
    Instruction, InstructionAuthority, Item, ItemId, Message, MessageRole, Part, PartId,
    Truncation,
};
pub use requirements::GenerationRequirements;
pub use resource::{Resource, ResourceKind, ResourceLocation};
pub use text::{
    Annotation, Logprob, TextContent, TopLogprob, compatible_logprobs, validate_logprobs,
};
pub use tool::{
    CustomCall, CustomFormat, CustomTool, FunctionStrictness, FunctionTool, GrammarSyntax,
    ItemLifecycle, StrictDefault, ToolCall, ToolChoice, ToolDefinition, ToolKind, ToolOutput,
    ToolReference, ToolResult,
};

pub use event::{
    EventError, ItemKind, PartKind, StreamEvent, StreamItem, StreamPart, StreamState,
    StreamTerminal, end_of_stream, materialize, reduce, snapshot_items,
};
pub use response::{
    Completion, GenerationResponse, IncompleteReason, Outcome, ResponseError, TerminalDetails,
    Usage,
};
pub use validate::{MAX_ITEMS, MAX_TEXT_BYTES, MAX_TOOLS, MAX_TOTAL_BYTES};
