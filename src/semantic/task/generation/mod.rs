//! Generation semantic IR.
mod output;
mod reasoning;
mod request;
mod requirements;
mod resource;
mod response;
mod tool;
mod validate;
pub use output::OutputConstraint;
pub use reasoning::{ReasoningEffort, ReasoningRequest, ReasoningSummary};
pub use request::{
    ContentPart, GenerationControls, GenerationError, GenerationRequest, Instruction,
    InstructionAuthority, Item, ItemId, Message, MessageRole, Part, PartId,
};
pub use requirements::GenerationRequirements;
pub use resource::{Resource, ResourceKind, ResourceLocation};
pub use tool::{
    FunctionStrictness, FunctionTool, StrictDefault, ToolCall, ToolChoice, ToolDefinition,
    ToolResult,
};

pub use response::{Completion, GenerationResponse};
pub use validate::{MAX_ITEMS, MAX_TEXT_BYTES, MAX_TOOLS, MAX_TOTAL_BYTES};
