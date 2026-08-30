//! Ordered canonical Generation request values.

use std::collections::BTreeSet;

use super::{
    GenerationControls, OutputConstraint, OutputProjection, ParallelToolCalls, ProviderExtension,
    ReasoningItem, ReasoningRequest, RequestState, Resource, TextContent, TextValue, ToolCall,
    ToolChoice, ToolDefinition, ToolKind, ToolResult, ValidationError,
};

/// Authority carried by a canonical instruction item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionAuthority {
    /// Highest-level system instruction.
    System,
    /// Application/developer instruction below the system authority.
    Developer,
}

/// Origin of one canonical instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionOrigin {
    /// Instruction decoded from the downstream request.
    Downstream,
    /// Instruction injected by trusted Gateway policy.
    GatewayPolicy,
    /// Instruction replayed from one upstream Provider origin.
    UpstreamProvider(super::ProviderOrigin),
}

/// Ordered request instruction with explicit authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    authority: InstructionAuthority,
    origin: InstructionOrigin,
    text: TextValue,
}

impl Instruction {
    /// Creates an instruction from already validated text.
    pub fn new(
        authority: InstructionAuthority,
        origin: InstructionOrigin,
        text: TextValue,
    ) -> Self {
        Self {
            authority,
            origin,
            text,
        }
    }

    /// Returns the instruction authority.
    pub fn authority(&self) -> InstructionAuthority {
        self.authority
    }

    /// Returns the instruction origin.
    pub const fn origin(&self) -> &InstructionOrigin {
        &self.origin
    }

    /// Returns the instruction text.
    pub fn text(&self) -> &TextValue {
        &self.text
    }
}

/// Semantic role of a canonical message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRole {
    /// User-provided content.
    User,
    /// Assistant history content.
    Assistant,
}

/// Portable content part shared by request messages and later tool outputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentPart {
    /// Plain text content.
    Text(TextContent),
    /// Typed image, audio, or file input.
    Resource(Resource),
}

impl ContentPart {
    /// Creates unannotated text content.
    pub fn text(text: TextValue) -> Self {
        Self::Text(text.into())
    }
}

/// One ordered conversation message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    role: MessageRole,
    content: Vec<ContentPart>,
}

impl Message {
    /// Creates a message and rejects an empty content list.
    pub fn new(role: MessageRole, content: Vec<ContentPart>) -> Result<Self, ValidationError> {
        if content.is_empty() {
            return Err(ValidationError::EmptyMessage);
        }
        Ok(Self { role, content })
    }

    /// Returns the message role.
    pub fn role(&self) -> MessageRole {
        self.role
    }

    /// Returns ordered message content.
    pub fn content(&self) -> &[ContentPart] {
        &self.content
    }
}
/// Ordered semantic input accepted by a Generation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputItem {
    /// Authority-bearing instruction.
    Instruction(Instruction),
    /// User or assistant message.
    Message(Message),
    /// A completed tool call retained in ordered conversation history.
    PriorToolCall(ToolCall),
    /// A completed tool result retained in ordered conversation history.
    ToolResult(ToolResult),
    /// Provider reasoning replay retained outside assistant message content.
    ReasoningReplay(ReasoningItem),
    /// Provider-private input item accepted only by an explicit target profile.
    Extension(ProviderExtension),
}

/// Static provider-neutral Generation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationRequest {
    input: Vec<InputItem>,
    tools: Vec<ToolDefinition>,
    tool_choice: ToolChoice,
    output: OutputConstraint,
    output_projection: OutputProjection,
    reasoning: ReasoningRequest,
    controls: GenerationControls,
    state: RequestState,
    extensions: Vec<ProviderExtension>,
}

impl GenerationRequest {
    /// Creates a request, preserving order and rejecting duplicate canonical history identities.
    pub fn new(input: Vec<InputItem>) -> Result<Self, ValidationError> {
        let mut item_ids = BTreeSet::new();
        let mut call_ids = BTreeSet::new();
        for item in &input {
            let item_id = match item {
                InputItem::PriorToolCall(call) => {
                    if !call_ids.insert(call.call_id().as_str()) {
                        return Err(ValidationError::DuplicateInputCallId {
                            id: call.call_id().as_str().to_owned(),
                        });
                    }
                    Some(call.id())
                }
                InputItem::ToolResult(result) => Some(result.id()),
                InputItem::ReasoningReplay(reasoning) => Some(reasoning.id()),
                InputItem::Instruction(_) | InputItem::Message(_) | InputItem::Extension(_) => None,
            };
            if let Some(item_id) = item_id
                && !item_ids.insert(item_id.as_str())
            {
                return Err(ValidationError::DuplicateInputItemId {
                    id: item_id.as_str().to_owned(),
                });
            }
        }
        Ok(Self {
            input,
            tools: Vec::new(),
            tool_choice: ToolChoice::None,
            output: OutputConstraint::Text,
            output_projection: OutputProjection::default(),
            reasoning: ReasoningRequest::default(),
            controls: GenerationControls::default(),
            state: RequestState::default(),
            extensions: Vec::new(),
        })
    }

    /// Returns the ordered semantic input.
    pub fn input(&self) -> &[InputItem] {
        &self.input
    }

    /// Appends canonical history while preserving every request control and revalidating identities.
    pub fn with_appended_input(
        mut self,
        items: impl IntoIterator<Item = InputItem>,
    ) -> Result<Self, ValidationError> {
        let mut input = self.input.clone();
        input.extend(items);
        Self::new(input.clone())?;
        self.input = input;
        Ok(self)
    }

    /// Adds a complete tool configuration after validating name and choice invariants.
    pub fn with_tools(
        mut self,
        tools: Vec<ToolDefinition>,
        tool_choice: ToolChoice,
        parallel_tool_calls: ParallelToolCalls,
    ) -> Result<Self, ValidationError> {
        let mut names = BTreeSet::new();
        for tool in &tools {
            if !names.insert(tool.name().as_str()) {
                return Err(ValidationError::DuplicateToolName {
                    name: tool.name().as_str().to_owned(),
                });
            }
        }

        let has_function = tools
            .iter()
            .any(|tool| matches!(tool.kind(), ToolKind::Function(_)));
        if parallel_tool_calls != ParallelToolCalls::Inactive
            && (!has_function || matches!(tool_choice, ToolChoice::None))
        {
            return Err(ValidationError::ParallelToolsWithoutFunction);
        }
        match &tool_choice {
            ToolChoice::None | ToolChoice::Auto if tools.is_empty() => {}
            ToolChoice::None | ToolChoice::Auto | ToolChoice::Required if !tools.is_empty() => {}
            ToolChoice::Specific(name) if tools.iter().any(|tool| tool.name() == name) => {}
            _ => return Err(ValidationError::InvalidToolChoice),
        }

        self.tools = tools;
        self.tool_choice = tool_choice;
        self.controls = self.controls.with_parallel_tool_calls(parallel_tool_calls);
        Ok(self)
    }

    /// Replaces the request's output constraint.
    pub fn with_output(mut self, output: OutputConstraint) -> Self {
        self.output = output;
        self
    }

    /// Replaces requested additional response projections.
    pub fn with_output_projection(mut self, projection: OutputProjection) -> Self {
        self.output_projection = projection;
        self
    }

    /// Replaces portable reasoning controls.
    pub fn with_reasoning(mut self, reasoning: ReasoningRequest) -> Self {
        self.reasoning = reasoning;
        self
    }

    /// Replaces portable generation controls.
    pub fn with_controls(mut self, controls: GenerationControls) -> Result<Self, ValidationError> {
        let has_function = self
            .tools
            .iter()
            .any(|tool| matches!(tool.kind(), ToolKind::Function(_)));
        if controls.parallel_tool_calls() != ParallelToolCalls::Inactive
            && (!has_function || matches!(self.tool_choice, ToolChoice::None))
        {
            return Err(ValidationError::ParallelToolsWithoutFunction);
        }
        self.controls = controls;
        Ok(self)
    }

    /// Replaces canonical request state.
    pub fn with_state(mut self, state: RequestState) -> Self {
        self.state = state;
        self
    }

    /// Replaces bounded Provider extensions.
    pub fn with_extensions(mut self, extensions: Vec<ProviderExtension>) -> Self {
        self.extensions = extensions;
        self
    }

    /// Returns the configured tools.
    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    /// Returns whether parallel function calls are requested.
    pub const fn parallel_tool_calls(&self) -> ParallelToolCalls {
        self.controls.parallel_tool_calls()
    }

    /// Returns the active tool-selection requirement.
    pub fn tool_choice(&self) -> &ToolChoice {
        &self.tool_choice
    }

    /// Returns the output constraint.
    pub fn output(&self) -> &OutputConstraint {
        &self.output
    }

    /// Returns requested additional response projections.
    pub fn output_projection(&self) -> &OutputProjection {
        &self.output_projection
    }

    /// Returns portable reasoning controls.
    pub const fn reasoning(&self) -> ReasoningRequest {
        self.reasoning
    }

    /// Returns portable generation controls.
    pub const fn controls(&self) -> &GenerationControls {
        &self.controls
    }

    /// Returns canonical request state.
    pub fn state(&self) -> &RequestState {
        &self.state
    }

    /// Returns bounded Provider extensions.
    pub fn extensions(&self) -> &[ProviderExtension] {
        &self.extensions
    }
}
