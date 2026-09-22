//! Generation semantic IR.
mod event;
mod output;
mod reasoning;
mod request;
mod requirements;
mod resource;
mod response;
mod tool;
mod validate;
pub use output::OutputConstraint;
pub use reasoning::{
    EncryptedReasoning, ReasoningContent, ReasoningEffort, ReasoningItem, ReasoningPresence,
    ReasoningReplay, ReasoningRequest, ReasoningSummary,
};
pub use request::{
    ContentPart, GenerationControls, GenerationError, GenerationRequest, Instruction,
    InstructionAuthority, Item, ItemId, Message, MessageRole, Part, PartId,
};
pub use requirements::GenerationRequirements;
pub use resource::{Resource, ResourceKind, ResourceLocation};
pub use tool::{
    FunctionStrictness, FunctionTool, ItemLifecycle, StrictDefault, ToolCall, ToolChoice,
    ToolDefinition, ToolResult,
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
