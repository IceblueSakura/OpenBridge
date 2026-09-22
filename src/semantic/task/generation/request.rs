//! Validated immutable Generation request and identity-preserving transformations.
use super::{
    OutputConstraint, ReasoningRequest, Resource, ToolCall, ToolChoice, ToolDefinition, ToolResult,
};
use crate::semantic::value::Text;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ItemId(u64);
impl ItemId {
    pub const fn new(v: u64) -> Self {
        Self(v)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PartId(u64);
impl PartId {
    pub const fn new(v: u64) -> Self {
        Self(v)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionAuthority {
    System,
    Developer,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    pub authority: InstructionAuthority,
    pub text: Text,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentPart {
    Text(Text),
    /// Assistant refusal, distinct from ordinary text and from a failed terminal.
    Refusal(Text),
    Resource(Resource),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Part {
    pub id: PartId,
    pub content: ContentPart,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub parts: Vec<Part>,
    pub status: super::ItemLifecycle,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Instruction(Instruction),
    Message(Message),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
    Reasoning(super::ReasoningItem),
}
impl Item {
    pub fn lifecycle(&self) -> Option<super::ItemLifecycle> {
        match self {
            Self::Message(m) => Some(m.status),
            Self::ToolCall(c) => Some(c.status),
            Self::Reasoning(r) => Some(r.status),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct GenerationControls {
    pub max_output_tokens: Option<u64>,
    temperature_bits: Option<u64>,
}
impl GenerationControls {
    pub fn with_temperature(mut self, v: f64) -> Result<Self, GenerationError> {
        if !v.is_finite() {
            return Err(GenerationError::NonFiniteTemperature);
        }
        self.temperature_bits = Some(v.to_bits());
        Ok(self)
    }
    pub fn temperature(&self) -> Option<f64> {
        self.temperature_bits.map(f64::from_bits)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationRequest {
    items: Vec<(ItemId, Item)>,
    controls: GenerationControls,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<ToolChoice>,
    parallel_tool_calls: Option<bool>,
    output: OutputConstraint,
    reasoning: ReasoningRequest,
}
impl GenerationRequest {
    pub fn new(
        items: Vec<(ItemId, Item)>,
        controls: GenerationControls,
    ) -> Result<Self, GenerationError> {
        let value = Self {
            items,
            controls,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            output: OutputConstraint::Text,
            reasoning: ReasoningRequest::default(),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), GenerationError> {
        if self.controls.max_output_tokens == Some(0) {
            return Err(GenerationError::InvalidControl);
        }
        if self.reasoning.effort() == Some(super::ReasoningEffort::None)
            && self
                .reasoning
                .summary()
                .is_some_and(|s| s != super::ReasoningSummary::Disabled)
        {
            return Err(GenerationError::InvalidControl);
        }
        let items = super::validate::items(&self.items, false)?;
        let tools = super::validate::tools(self.tools(), self.tool_choice.as_ref())?;
        if items.saturating_add(tools) > super::MAX_TOTAL_BYTES {
            return Err(GenerationError::Limit);
        }
        Ok(())
    }
    pub fn items(&self) -> &[(ItemId, Item)] {
        &self.items
    }
    pub const fn controls(&self) -> &GenerationControls {
        &self.controls
    }
    pub fn tools(&self) -> &[ToolDefinition] {
        self.tools.as_deref().unwrap_or_default()
    }
    pub const fn tools_present(&self) -> bool {
        self.tools.is_some()
    }
    pub const fn tool_choice(&self) -> Option<&ToolChoice> {
        self.tool_choice.as_ref()
    }
    pub const fn parallel_tool_calls(&self) -> Option<bool> {
        self.parallel_tool_calls
    }
    pub const fn output(&self) -> &OutputConstraint {
        &self.output
    }
    pub const fn reasoning(&self) -> &ReasoningRequest {
        &self.reasoning
    }
    pub fn with_tool_settings(
        mut self,
        tools: Option<Vec<ToolDefinition>>,
        choice: Option<ToolChoice>,
        parallel: Option<bool>,
    ) -> Result<Self, GenerationError> {
        self.tools = tools;
        self.tool_choice = choice;
        self.parallel_tool_calls = parallel;
        self.validate()?;
        Ok(self)
    }
    pub fn with_tools(
        self,
        tools: Vec<ToolDefinition>,
        choice: ToolChoice,
    ) -> Result<Self, GenerationError> {
        let parallel = self.parallel_tool_calls;
        self.with_tool_settings(Some(tools), Some(choice), parallel)
    }
    pub fn with_output(mut self, output: OutputConstraint) -> Self {
        self.output = output;
        self
    }
    pub fn with_reasoning(mut self, reasoning: ReasoningRequest) -> Self {
        self.reasoning = reasoning;
        self
    }
    /// Replacement, insertion and reordering are validated together so references cannot dangle.
    pub fn with_items(mut self, items: Vec<(ItemId, Item)>) -> Result<Self, GenerationError> {
        self.items = items;
        self.validate()?;
        Ok(self)
    }
    pub fn retain_items(
        self,
        mut keep: impl FnMut(ItemId, &Item) -> bool,
    ) -> Result<Self, GenerationError> {
        let items = self
            .items
            .iter()
            .filter(|(id, item)| keep(*id, item))
            .cloned()
            .collect();
        self.with_items(items)
    }
}
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum GenerationError {
    #[error("generation input must not be empty")]
    EmptyInput,
    #[error("duplicate item identity")]
    DuplicateItemId,
    #[error("user message must contain at least one content part")]
    EmptyMessage,
    #[error("duplicate part identity")]
    DuplicatePartId,
    #[error("temperature must be finite")]
    NonFiniteTemperature,
    #[error("invalid generation control")]
    InvalidControl,
    #[error("tool call identity is duplicated")]
    DuplicateCall,
    #[error("tool result has no preceding call or duplicates a result")]
    InvalidToolResult,
    #[error("tool calls must remain contiguous with their assistant message owner")]
    InvalidMessageGroup,
    #[error("function definitions must have unique names and bounded object schemas")]
    InvalidToolDefinition,
    #[error("tool choice refers to an unavailable function")]
    InvalidToolChoice,
    #[error("semantic value exceeds the supported slice limits")]
    Limit,
    #[error("unsupported static response semantics")]
    InvalidResponse,
    #[error("refusal content is only valid for assistant messages")]
    RefusalInUserMessage,
}
