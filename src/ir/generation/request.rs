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
    /// Model refusal text kept separate from ordinary assistant text.
    Refusal(TextValue),
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
    part_ids: Vec<usize>,
}

impl Message {
    /// Creates a message and rejects an empty content list.
    pub fn new(role: MessageRole, content: Vec<ContentPart>) -> Result<Self, ValidationError> {
        if content.is_empty() {
            return Err(ValidationError::EmptyMessage);
        }
        if role == MessageRole::User
            && content
                .iter()
                .any(|part| matches!(part, ContentPart::Refusal(_)))
        {
            return Err(ValidationError::RefusalInUserMessage);
        }
        Ok(Self {
            role,
            part_ids: (0..content.len()).collect(),
            content,
        })
    }

    /// Returns the message role.
    pub fn role(&self) -> MessageRole {
        self.role
    }

    /// Returns ordered message content.
    pub fn content(&self) -> &[ContentPart] {
        &self.content
    }

    /// Returns stable message-local part identities in current order.
    pub fn part_ids(&self) -> &[usize] {
        &self.part_ids
    }

    /// Replaces message parts with explicit identities, validating role and uniqueness.
    pub fn with_identified_content(
        self,
        parts: Vec<(usize, ContentPart)>,
    ) -> Result<Self, ValidationError> {
        let (ids, content): (Vec<_>, Vec<_>) = parts.into_iter().unzip();
        if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(ValidationError::InvalidInputLayout);
        }
        let mut result = Self::new(self.role, content)?;
        result.part_ids = ids;
        Ok(result)
    }
}

/// Stable request-local item identity and its explicit message group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputIdentity {
    id: usize,
    group: usize,
}

impl InputIdentity {
    /// Creates an identity; insertion into a request validates uniqueness and group contiguity.
    pub const fn new(id: usize, group: usize) -> Self {
        Self { id, group }
    }
    /// Returns the stable local item identity, not its current array index.
    pub const fn id(self) -> usize {
        self.id
    }
    /// Returns explicit message membership, not inferred adjacency.
    pub const fn group(self) -> usize {
        self.group
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
    input_identities: Vec<InputIdentity>,
    identity_high_watermark: Option<usize>,
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
            input_identities: (0..input.len())
                .map(|id| InputIdentity::new(id, id))
                .collect(),
            identity_high_watermark: input.len().checked_sub(1),
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

    /// Returns stable identities and explicit message grouping for the ordered input.
    pub fn input_identities(&self) -> &[InputIdentity] {
        &self.input_identities
    }

    /// Applies an ordered edit without losing controls or silently reassigning source identity.
    pub fn with_identified_input(
        mut self,
        entries: Vec<(InputIdentity, InputItem)>,
    ) -> Result<Self, ValidationError> {
        let (identities, input): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        Self::new(input.clone())?;
        let mut ids = BTreeSet::new();
        let mut groups = BTreeSet::new();
        let mut previous = None;
        for identity in &identities {
            if !ids.insert(identity.id)
                || (previous != Some(identity.group) && !groups.insert(identity.group))
            {
                return Err(ValidationError::InvalidInputLayout);
            }
            previous = Some(identity.group);
        }
        self.input = input;
        self.identity_high_watermark = self.identity_high_watermark.max(
            identities
                .iter()
                .map(|identity| identity.id.max(identity.group))
                .max(),
        );
        self.input_identities = identities;
        Ok(self)
    }

    /// Appends canonical history while preserving every request control and revalidating identities.
    pub fn with_appended_input(
        mut self,
        items: impl IntoIterator<Item = InputItem>,
    ) -> Result<Self, ValidationError> {
        let mut entries: Vec<_> = self
            .input_identities
            .iter()
            .copied()
            .zip(self.input.iter().cloned())
            .collect();
        let mut next_id = self.identity_high_watermark;
        for item in items {
            let id = match next_id {
                None => 0,
                Some(previous) => previous
                    .checked_add(1)
                    .ok_or(ValidationError::InvalidInputLayout)?,
            };
            next_id = Some(id);
            entries.push((InputIdentity::new(id, id), item));
        }
        self = self.with_identified_input(entries)?;
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
