//! Canonical response identity values.
//!
//! These leaf types keep Gateway correlation identities distinct from Provider provenance. They
//! validate only their local, caller-supplied byte bound; routing, replay, and wire semantics stay
//! outside the Static IR.

use std::{borrow::Borrow, collections::BTreeSet, fmt, ops::Deref};

use thiserror::Error;

use super::{
    ContentPart, JsonObject, OpaqueState, ProviderExtension, Resource, Source, SourceId,
    TextAnnotation, TextValue, ToolInput, ToolName, WireIdentity,
};

/// Validation failure for a bounded response or Provider identity value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdentityValidationError {
    /// The value contains no bytes.
    #[error("identity must not be empty")]
    Empty,
    /// The value exceeds the caller-supplied UTF-8 byte bound.
    #[error("identity exceeds the {max_bytes}-byte limit")]
    TooLarge {
        /// Maximum accepted UTF-8 bytes.
        max_bytes: usize,
    },
}

impl IdentityValidationError {
    fn validate(value: String, max_bytes: usize) -> Result<String, Self> {
        if value.is_empty() {
            return Err(Self::Empty);
        }
        if value.len() > max_bytes {
            return Err(Self::TooLarge { max_bytes });
        }
        Ok(value)
    }
}

macro_rules! bounded_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Creates a non-empty identity within the supplied UTF-8 byte bound.
            pub fn new(
                value: impl Into<String>,
                max_bytes: usize,
            ) -> Result<Self, IdentityValidationError> {
                IdentityValidationError::validate(value.into(), max_bytes).map(Self)
            }

            /// Returns the validated identity text.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the identity and returns its validated text.
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

bounded_identity! {
    /// Gateway identity for one complete Generation response.
    ResponseId
}

bounded_identity! {
    /// Gateway identity for one response candidate.
    CandidateId
}

bounded_identity! {
    /// Gateway identity for one ordered response item.
    ItemId
}

bounded_identity! {
    /// Gateway correlation identity for one tool call.
    CallId
}

/// Validation failure for one canonical response value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResponseValidationError {
    /// A message output contains no content.
    #[error("response message must contain at least one content part")]
    EmptyMessage,
    /// A reasoning output contains no parts.
    #[error("reasoning output must contain at least one part")]
    EmptyReasoning,
    /// Candidate identity occurs more than once.
    #[error("duplicate candidate identity '{id}'")]
    DuplicateCandidateId {
        /// Duplicate candidate identity.
        id: CandidateId,
    },
    /// Item identity occurs more than once within one candidate.
    #[error("duplicate item identity '{id}'")]
    DuplicateItemId {
        /// Duplicate item identity.
        id: ItemId,
    },
    /// Tool-call identity occurs more than once within one candidate.
    #[error("duplicate call identity '{id}'")]
    DuplicateCallId {
        /// Duplicate call identity.
        id: CallId,
    },
    /// Source identity occurs more than once within one candidate.
    #[error("duplicate source identity '{id}'")]
    DuplicateSourceId {
        /// Duplicate source identity.
        id: SourceId,
    },
    /// A text annotation refers to no source in the candidate.
    #[error("unknown source reference '{id}'")]
    UnknownSourceReference {
        /// Unresolved source identity.
        id: SourceId,
    },
}

/// Human-readable or opaque reasoning output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReasoningPart {
    /// Complete readable reasoning text.
    Visible(TextValue),
    /// Human-readable reasoning summary.
    Summary(TextValue),
    /// Provider-owned replay state.
    Opaque(OpaqueState),
}

/// One assistant message output item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseMessage {
    id: ItemId,
    content: Vec<ContentPart>,
    wire_identity: Option<WireIdentity>,
}

impl ResponseMessage {
    /// Creates an ordered response message.
    pub fn new(
        id: ItemId,
        content: Vec<ContentPart>,
        wire_identity: Option<WireIdentity>,
    ) -> Result<Self, ResponseValidationError> {
        if content.is_empty() {
            return Err(ResponseValidationError::EmptyMessage);
        }
        Ok(Self {
            id,
            content,
            wire_identity,
        })
    }

    /// Returns the canonical item identity.
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns ordered content.
    pub fn content(&self) -> &[ContentPart] {
        &self.content
    }

    /// Returns the Provider wire identity when one exists.
    pub const fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}

/// One reasoning output item kept separate from visible assistant text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningItem {
    id: ItemId,
    parts: Vec<ReasoningPart>,
    wire_identity: Option<WireIdentity>,
}

impl ReasoningItem {
    /// Creates reasoning output and rejects an empty part list.
    pub fn new(
        id: ItemId,
        parts: Vec<ReasoningPart>,
        wire_identity: Option<WireIdentity>,
    ) -> Result<Self, ResponseValidationError> {
        if parts.is_empty() {
            return Err(ResponseValidationError::EmptyReasoning);
        }
        Ok(Self {
            id,
            parts,
            wire_identity,
        })
    }

    /// Returns the canonical item identity.
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns ordered reasoning parts.
    pub fn parts(&self) -> &[ReasoningPart] {
        &self.parts
    }

    /// Returns the Provider wire identity when one exists.
    pub const fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}

/// One model-emitted tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCall {
    id: ItemId,
    call_id: CallId,
    tool: ToolName,
    input: ToolInput,
    wire_identity: Option<WireIdentity>,
}

impl ToolCall {
    /// Creates a tool call from validated identity and input values.
    pub fn new(
        id: ItemId,
        call_id: CallId,
        tool: ToolName,
        input: ToolInput,
        wire_identity: Option<WireIdentity>,
    ) -> Self {
        Self {
            id,
            call_id,
            tool,
            input,
            wire_identity,
        }
    }

    /// Returns the canonical item identity.
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns the semantic tool-call identity.
    pub fn call_id(&self) -> &CallId {
        &self.call_id
    }

    /// Returns the canonical tool name.
    pub const fn tool(&self) -> &ToolName {
        &self.tool
    }

    /// Returns the completed typed tool input.
    pub const fn input(&self) -> &ToolInput {
        &self.input
    }

    /// Returns the Provider wire identity when one exists.
    pub const fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}

/// Terminal status of one tool execution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolResultStatus {
    /// Tool execution succeeded.
    Success,
    /// Tool execution returned an application error.
    Error,
    /// Tool execution was denied by an approval boundary.
    Denied,
}

/// Structured tool result payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolOutput {
    /// Plain text output.
    Text(TextValue),
    /// Structured JSON object output.
    Json(JsonObject),
    /// Ordered multimodal output.
    Content(Vec<ContentPart>),
    /// Typed image, audio, or file output.
    Resource(Resource),
    /// Public source/citation output.
    Source(Source),
}

/// One tool result correlated to a prior call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolResult {
    id: ItemId,
    call_id: CallId,
    status: ToolResultStatus,
    output: Vec<ToolOutput>,
    wire_identity: Option<WireIdentity>,
}

impl ToolResult {
    /// Creates a tool result.
    pub fn new(
        id: ItemId,
        call_id: CallId,
        status: ToolResultStatus,
        output: Vec<ToolOutput>,
        wire_identity: Option<WireIdentity>,
    ) -> Self {
        Self {
            id,
            call_id,
            status,
            output,
            wire_identity,
        }
    }

    /// Returns the canonical item identity.
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns the correlated call identity.
    pub fn call_id(&self) -> &CallId {
        &self.call_id
    }

    /// Returns the terminal tool-result status.
    pub const fn status(&self) -> ToolResultStatus {
        self.status
    }

    /// Returns ordered result output values.
    pub fn output(&self) -> &[ToolOutput] {
        &self.output
    }

    /// Returns the Provider wire identity when one exists.
    pub fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}

/// Provider-private ordered output item accepted only by an explicit target profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionOutput {
    id: ItemId,
    extension: ProviderExtension,
    wire_identity: Option<WireIdentity>,
}

impl ExtensionOutput {
    /// Creates a Provider extension output item.
    pub const fn new(
        id: ItemId,
        extension: ProviderExtension,
        wire_identity: Option<WireIdentity>,
    ) -> Self {
        Self {
            id,
            extension,
            wire_identity,
        }
    }

    /// Returns the canonical item identity.
    pub const fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns the bounded Provider extension.
    pub const fn extension(&self) -> &ProviderExtension {
        &self.extension
    }

    /// Returns the Provider wire identity when one exists.
    pub const fn wire_identity(&self) -> Option<&WireIdentity> {
        self.wire_identity.as_ref()
    }
}

/// Ordered canonical output item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputItem {
    /// Assistant message.
    Message(ResponseMessage),
    /// Reasoning output.
    Reasoning(ReasoningItem),
    /// Tool call.
    ToolCall(ToolCall),
    /// Tool execution result.
    ToolResult(ToolResult),
    /// Public source/citation item.
    Source(Source),
    /// Provider-private extension item.
    Extension(ExtensionOutput),
}

impl OutputItem {
    fn id(&self) -> &ItemId {
        match self {
            Self::Message(value) => value.id(),
            Self::Reasoning(value) => value.id(),
            Self::ToolCall(value) => value.id(),
            Self::ToolResult(value) => value.id(),
            Self::Source(value) => value.item_id(),
            Self::Extension(value) => value.id(),
        }
    }
}

/// Semantic finish reason for one candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinishReason {
    /// Model completed normally.
    Stop,
    /// Output limit was reached.
    Length,
    /// Model emitted one or more tool calls.
    ToolCalls,
    /// Provider content filter stopped generation.
    ContentFilter,
    /// Provider-specific finish reason retained as an opaque extension label.
    Extension(TextValue),
}

fn insert_source(
    source: &Source,
    source_ids: &mut BTreeSet<SourceId>,
) -> Result<(), ResponseValidationError> {
    if source_ids.insert(source.id().clone()) {
        Ok(())
    } else {
        Err(ResponseValidationError::DuplicateSourceId {
            id: source.id().clone(),
        })
    }
}

fn collect_sources(
    item: &OutputItem,
    source_ids: &mut BTreeSet<SourceId>,
) -> Result<(), ResponseValidationError> {
    match item {
        OutputItem::Source(source) => insert_source(source, source_ids),
        OutputItem::ToolResult(result) => {
            for output in result.output() {
                if let ToolOutput::Source(source) = output {
                    insert_source(source, source_ids)?;
                }
            }
            Ok(())
        }
        OutputItem::Message(_)
        | OutputItem::Reasoning(_)
        | OutputItem::ToolCall(_)
        | OutputItem::Extension(_) => Ok(()),
    }
}

fn validate_content_sources(
    content: &[ContentPart],
    source_ids: &BTreeSet<SourceId>,
) -> Result<(), ResponseValidationError> {
    for part in content {
        if let ContentPart::Text(text) = part {
            for annotation in text.annotations() {
                let TextAnnotation::Citation(reference) = annotation;
                if !source_ids.contains(reference.id()) {
                    return Err(ResponseValidationError::UnknownSourceReference {
                        id: reference.id().clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_source_references(
    item: &OutputItem,
    source_ids: &BTreeSet<SourceId>,
) -> Result<(), ResponseValidationError> {
    match item {
        OutputItem::Message(message) => validate_content_sources(message.content(), source_ids),
        OutputItem::ToolResult(result) => {
            for output in result.output() {
                if let ToolOutput::Content(content) = output {
                    validate_content_sources(content, source_ids)?;
                }
            }
            Ok(())
        }
        OutputItem::Reasoning(_)
        | OutputItem::ToolCall(_)
        | OutputItem::Source(_)
        | OutputItem::Extension(_) => Ok(()),
    }
}

/// One ordered response candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    id: CandidateId,
    output: Vec<OutputItem>,
    finish: Option<FinishReason>,
}

impl Candidate {
    /// Creates a candidate and rejects duplicate item/call identities.
    pub fn new(
        id: CandidateId,
        output: Vec<OutputItem>,
        finish: Option<FinishReason>,
    ) -> Result<Self, ResponseValidationError> {
        let mut item_ids = BTreeSet::new();
        let mut call_ids = BTreeSet::new();
        let mut source_ids = BTreeSet::new();
        for item in &output {
            if !item_ids.insert(item.id().clone()) {
                return Err(ResponseValidationError::DuplicateItemId {
                    id: item.id().clone(),
                });
            }
            if let OutputItem::ToolCall(tool_call) = item
                && !call_ids.insert(tool_call.call_id().clone())
            {
                return Err(ResponseValidationError::DuplicateCallId {
                    id: tool_call.call_id().clone(),
                });
            }
            collect_sources(item, &mut source_ids)?;
        }
        for item in &output {
            validate_source_references(item, &source_ids)?;
        }
        Ok(Self { id, output, finish })
    }

    /// Returns the candidate identity.
    pub fn id(&self) -> &CandidateId {
        &self.id
    }

    /// Returns ordered output items.
    pub fn output(&self) -> &[OutputItem] {
        &self.output
    }

    /// Returns the semantic finish reason when one was observed.
    pub fn finish(&self) -> Option<&FinishReason> {
        self.finish.as_ref()
    }
}

/// Canonical response lifecycle status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseStatus {
    /// Response completed successfully.
    Completed,
    /// Response stopped before completing.
    Incomplete,
    /// Provider reported failure.
    Failed,
    /// Request was cancelled.
    Cancelled,
}

/// Optional Provider-reported token usage; absence is distinct from zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
}

impl Usage {
    /// Creates one usage snapshot without estimating missing values.
    pub const fn new(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
        reasoning_tokens: Option<u64>,
        cached_input_tokens: Option<u64>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens,
            reasoning_tokens,
            cached_input_tokens,
        }
    }

    /// Returns Provider-reported input tokens.
    pub const fn input_tokens(&self) -> Option<u64> {
        self.input_tokens
    }

    /// Returns Provider-reported output tokens.
    pub const fn output_tokens(&self) -> Option<u64> {
        self.output_tokens
    }

    /// Returns Provider-reported total tokens without estimating missing values.
    pub const fn total_tokens(&self) -> Option<u64> {
        self.total_tokens
    }

    /// Returns Provider-reported reasoning tokens.
    pub const fn reasoning_tokens(&self) -> Option<u64> {
        self.reasoning_tokens
    }

    /// Returns Provider-reported cached input tokens.
    pub const fn cached_input_tokens(&self) -> Option<u64> {
        self.cached_input_tokens
    }
}

/// Complete provider-neutral static Generation response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationResponse {
    id: ResponseId,
    candidates: Vec<Candidate>,
    status: ResponseStatus,
    usage: Option<Usage>,
    extensions: Vec<ProviderExtension>,
}

impl GenerationResponse {
    /// Creates a response and rejects duplicate candidate identities.
    pub fn new(
        id: ResponseId,
        candidates: Vec<Candidate>,
        status: ResponseStatus,
        usage: Option<Usage>,
        extensions: Vec<ProviderExtension>,
    ) -> Result<Self, ResponseValidationError> {
        let mut ids = BTreeSet::new();
        for candidate in &candidates {
            if !ids.insert(candidate.id().clone()) {
                return Err(ResponseValidationError::DuplicateCandidateId {
                    id: candidate.id().clone(),
                });
            }
        }
        Ok(Self {
            id,
            candidates,
            status,
            usage,
            extensions,
        })
    }

    /// Returns ordered response candidates.
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Returns the canonical response identity.
    pub fn id(&self) -> &ResponseId {
        &self.id
    }

    /// Returns the terminal static response status.
    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    /// Returns Provider-reported usage when present.
    pub const fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }

    /// Returns bounded Provider extensions.
    pub fn extensions(&self) -> &[ProviderExtension] {
        &self.extensions
    }
}
