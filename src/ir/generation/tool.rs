//! Canonical tool declarations and output constraints.

use super::{
    JsonObject, JsonSchema, ProviderExtension, ProviderOrigin, TextValue, ToolName, ToolPlanId,
};

/// Origin of one canonical tool declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolOrigin {
    /// Tool declared by the downstream request.
    Downstream,
    /// Tool injected by one trusted Gateway policy plan.
    GatewayPolicy(ToolPlanId),
    /// Tool declaration observed from one upstream Provider origin.
    UpstreamProvider(ProviderOrigin),
}

/// Owner responsible for executing a canonical tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolExecutor {
    /// The downstream client executes the tool.
    Client,
    /// OpenBridge executes the tool in a later bounded Gateway loop.
    Gateway,
    /// The bound upstream Provider executes the tool.
    Provider(ProviderOrigin),
}

/// Downstream visibility of one tool lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolVisibility {
    /// Tool declarations, calls, and results are part of the public interaction.
    Public,
    /// Tool lifecycle is internal to a later Gateway execution loop.
    Internal,
}

/// Standard JSON function-tool definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionTool {
    description: Option<TextValue>,
    parameters: JsonSchema,
    strict: bool,
}

impl FunctionTool {
    /// Creates a function tool from validated portable values.
    pub fn new(description: Option<TextValue>, parameters: JsonSchema, strict: bool) -> Self {
        Self {
            description,
            parameters,
            strict,
        }
    }

    /// Returns whether the function schema requests strict validation.
    pub fn strict(&self) -> bool {
        self.strict
    }

    /// Returns the optional client-facing description.
    pub fn description(&self) -> Option<&TextValue> {
        self.description.as_ref()
    }

    /// Returns the validated input schema.
    pub fn parameters(&self) -> &JsonSchema {
        &self.parameters
    }
}

/// Closed portable server-tool kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ServerToolKind {
    /// Provider- or Gateway-executed web search.
    WebSearch,
    /// Provider- or Gateway-executed file search.
    FileSearch,
    /// Sandboxed code execution.
    CodeExecution,
    /// Computer-use execution.
    ComputerUse,
    /// Image generation exposed as a tool.
    ImageGeneration,
    /// Remote MCP invocation.
    Mcp,
    /// Hosted shell execution.
    Shell,
    /// Apply-patch execution.
    ApplyPatch,
    /// Provider tool discovery.
    ToolSearch,
    /// Provider skill invocation.
    Skills,
    /// Provider programmatic tool calling.
    ProgrammaticToolCalling,
}

/// Typed server-tool declaration retained independently from its executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerToolConfig {
    /// Web search with Provider/Gateway-specific configuration carried only by an extension.
    WebSearch,
    /// File search.
    FileSearch,
    /// Code execution.
    CodeExecution,
    /// Computer use.
    ComputerUse,
    /// Image generation.
    ImageGeneration,
    /// Remote MCP invocation.
    Mcp,
    /// Hosted shell execution.
    Shell,
    /// Apply-patch execution.
    ApplyPatch,
    /// Provider tool discovery.
    ToolSearch,
    /// Provider skill invocation.
    Skills,
    /// Provider programmatic tool calling.
    ProgrammaticToolCalling,
}

impl ServerToolConfig {
    /// Returns the capability kind without interpreting Provider configuration.
    pub const fn kind(&self) -> ServerToolKind {
        match self {
            Self::WebSearch => ServerToolKind::WebSearch,
            Self::FileSearch => ServerToolKind::FileSearch,
            Self::CodeExecution => ServerToolKind::CodeExecution,
            Self::ComputerUse => ServerToolKind::ComputerUse,
            Self::ImageGeneration => ServerToolKind::ImageGeneration,
            Self::Mcp => ServerToolKind::Mcp,
            Self::Shell => ServerToolKind::Shell,
            Self::ApplyPatch => ServerToolKind::ApplyPatch,
            Self::ToolSearch => ServerToolKind::ToolSearch,
            Self::Skills => ServerToolKind::Skills,
            Self::ProgrammaticToolCalling => ServerToolKind::ProgrammaticToolCalling,
        }
    }
}

/// Completed typed input for one server tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerToolInput {
    kind: ServerToolKind,
    payload: JsonObject,
}

impl ServerToolInput {
    /// Creates typed server input from a closed kind and bounded JSON payload.
    pub const fn new(kind: ServerToolKind, payload: JsonObject) -> Self {
        Self { kind, payload }
    }

    /// Returns the server-tool kind.
    pub const fn kind(&self) -> ServerToolKind {
        self.kind
    }

    /// Returns the completed bounded payload.
    pub const fn payload(&self) -> &JsonObject {
        &self.payload
    }
}

/// Completed Static IR input for one tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolInput {
    /// Completed function arguments parsed as a bounded JSON object.
    Function(JsonObject),
    /// Typed server-tool input.
    Server(ServerToolInput),
    /// Provider-private input accepted only by an explicit target profile.
    Extension(ProviderExtension),
}

/// Portable kind of one tool declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolKind {
    /// Client/Gateway callable JSON function.
    Function(FunctionTool),
    /// Typed Provider- or Gateway-executed server tool.
    Server(ServerToolConfig),
    /// Provider-private tool declaration accepted only by an explicit target profile.
    Extension(ProviderExtension),
}

/// One canonical tool declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolDefinition {
    name: ToolName,
    origin: ToolOrigin,
    executor: ToolExecutor,
    visibility: ToolVisibility,
    kind: ToolKind,
}

impl ToolDefinition {
    /// Creates one tool declaration from already validated leaf values.
    pub fn new(
        name: ToolName,
        origin: ToolOrigin,
        executor: ToolExecutor,
        visibility: ToolVisibility,
        kind: ToolKind,
    ) -> Self {
        Self {
            name,
            origin,
            executor,
            visibility,
            kind,
        }
    }

    /// Returns the canonical tool name.
    pub fn name(&self) -> &ToolName {
        &self.name
    }

    /// Returns the portable tool kind.
    pub fn kind(&self) -> &ToolKind {
        &self.kind
    }

    /// Returns the declaration origin.
    pub fn origin(&self) -> &ToolOrigin {
        &self.origin
    }

    /// Returns the executor owner.
    pub fn executor(&self) -> &ToolExecutor {
        &self.executor
    }

    /// Returns downstream visibility.
    pub const fn visibility(&self) -> ToolVisibility {
        self.visibility
    }
}

/// Model tool-selection requirement.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ToolChoice {
    /// No tools may be called.
    None,
    /// The model may choose whether to call a tool.
    #[default]
    Auto,
    /// At least one configured tool must be called.
    Required,
    /// One named tool must be called.
    Specific(ToolName),
}

/// Capability-relevant tool-choice mode without the selected name payload.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToolChoiceRequirement {
    /// Tools are disabled.
    None,
    /// The model may choose whether to call a tool.
    #[default]
    Auto,
    /// At least one tool must be called.
    Required,
    /// One named tool is selected.
    Named,
}

impl ToolChoice {
    /// Returns the capability-relevant choice mode.
    pub const fn requirement(&self) -> ToolChoiceRequirement {
        match self {
            Self::None => ToolChoiceRequirement::None,
            Self::Auto => ToolChoiceRequirement::Auto,
            Self::Required => ToolChoiceRequirement::Required,
            Self::Specific(_) => ToolChoiceRequirement::Named,
        }
    }
}

/// Effective parallel-function-call requirement after semantic normalization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ParallelToolCalls {
    /// No active semantic requirement remains.
    #[default]
    Inactive,
    /// Active function tools may be emitted in parallel.
    Allow,
    /// Active function tools must remain serial.
    RequireSerial,
}

/// Canonical output constraint.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum OutputConstraint {
    /// Unconstrained text output.
    #[default]
    Text,
    /// A syntactically valid JSON object.
    JsonObject,
    /// Output validated against a named JSON Schema.
    JsonSchema {
        /// Stable schema name exposed to protocols that support one.
        name: TextValue,
        /// Optional human-readable schema description.
        description: Option<TextValue>,
        /// Validated portable JSON Schema.
        schema: JsonSchema,
        /// Whether exact schema adherence is required.
        strict: bool,
    },
}

/// Structured-output capability projected from a canonical request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StructuredOutputRequirement {
    /// No structured-output requirement.
    #[default]
    Text,
    /// Any valid JSON object.
    JsonObject,
    /// JSON Schema output with explicit strictness.
    JsonSchema {
        /// Whether strict schema adherence is required.
        strict: bool,
    },
}
