//! Pure semantic requirements projection from canonical requests.

use std::collections::BTreeSet;

use crate::core::ResponseInclude;

use super::{
    ContentPart, GenerationControls, GenerationRequest, ImageDetail, InputItem, MediaType,
    OutputConstraint, ParallelToolCalls, ReasoningEffort, ReasoningPresence, ReasoningRequest,
    ReasoningSummary, Resource, ResourceKind, ResourceSource, ServerToolKind, SourceLocation,
    StructuredOutputRequirement, ToolChoiceRequirement, ToolInput, ToolKind, ToolOutput,
    ToolResult,
};

/// Portable resource requirements for one media kind.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceRequirements {
    count: usize,
    url_sources: usize,
    inline_sources: usize,
    provider_references: usize,
    max_url_bytes: usize,
    total_inline_bytes: usize,
    media_types: BTreeSet<MediaType>,
    image_details: BTreeSet<ImageDetail>,
}

impl ResourceRequirements {
    /// Returns the number of resources of this kind.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the number of URL sources.
    pub const fn url_sources(&self) -> usize {
        self.url_sources
    }

    /// Returns the number of inline sources.
    pub const fn inline_sources(&self) -> usize {
        self.inline_sources
    }

    /// Returns the number of origin-bound Provider references.
    pub const fn provider_references(&self) -> usize {
        self.provider_references
    }

    /// Returns the largest canonical URL length in UTF-8 bytes.
    pub const fn max_url_bytes(&self) -> usize {
        self.max_url_bytes
    }

    /// Returns total decoded inline bytes without estimating source encoding overhead.
    pub const fn total_inline_bytes(&self) -> usize {
        self.total_inline_bytes
    }

    /// Returns declared media types.
    pub const fn media_types(&self) -> &BTreeSet<MediaType> {
        &self.media_types
    }

    /// Returns requested image-detail values; empty for non-image kinds.
    pub const fn image_details(&self) -> &BTreeSet<ImageDetail> {
        &self.image_details
    }
}

/// Semantic input requirements projected from ordered canonical items.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputRequirements {
    instructions: bool,
    text_parts: usize,
    image: ResourceRequirements,
    audio: ResourceRequirements,
    file: ResourceRequirements,
}

impl InputRequirements {
    /// Returns whether the request contains authority-bearing instructions.
    pub fn instructions(&self) -> bool {
        self.instructions
    }

    /// Returns the number of portable text content parts in messages.
    pub fn text_parts(&self) -> usize {
        self.text_parts
    }

    /// Returns whether image input is requested.
    pub fn image_input(&self) -> bool {
        self.image.count() > 0
    }

    /// Returns whether audio input is requested.
    pub fn audio_input(&self) -> bool {
        self.audio.count() > 0
    }

    /// Returns whether file input is requested.
    pub fn file_input(&self) -> bool {
        self.file.count() > 0
    }

    /// Returns image requirements.
    pub const fn image(&self) -> &ResourceRequirements {
        &self.image
    }

    /// Returns audio requirements.
    pub const fn audio(&self) -> &ResourceRequirements {
        &self.audio
    }

    /// Returns file requirements.
    pub const fn file(&self) -> &ResourceRequirements {
        &self.file
    }
}

/// Semantic tool requirements projected from canonical declarations and controls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ToolRequirements {
    function_tools: bool,
    strict_function_tools: bool,
    function_tool_count: usize,
    function_schema_bytes: usize,
    server_tools: BTreeSet<ServerToolKind>,
    server_history: BTreeSet<ServerToolKind>,
    tool_history: bool,
    tool_result_sources: usize,
    choice: ToolChoiceRequirement,
    parallel_tool_calls: ParallelToolCalls,
}

impl ToolRequirements {
    /// Returns whether standard function-tool semantics are requested.
    pub fn function_tools(&self) -> bool {
        self.function_tools
    }

    /// Returns whether any function definition requires strict JSON Schema validation.
    pub fn strict_function_tools(&self) -> bool {
        self.strict_function_tools
    }

    /// Returns the number of function declarations.
    pub const fn function_tool_count(&self) -> usize {
        self.function_tool_count
    }

    /// Returns total canonical compact-JSON schema bytes.
    pub const fn function_schema_bytes(&self) -> usize {
        self.function_schema_bytes
    }

    /// Returns whether parallel function calls are requested.
    pub const fn parallel_tool_calls(&self) -> ParallelToolCalls {
        self.parallel_tool_calls
    }

    /// Returns typed server-tool kinds requested by declarations.
    pub fn server_tools(&self) -> &BTreeSet<ServerToolKind> {
        &self.server_tools
    }

    /// Returns server-tool kinds present in ordered call history.
    pub fn server_history(&self) -> &BTreeSet<ServerToolKind> {
        &self.server_history
    }

    /// Returns whether ordered tool-call/result history is present.
    pub fn tool_history(&self) -> bool {
        self.tool_history
    }

    /// Returns public source outputs carried by historical tool results.
    pub const fn tool_result_sources(&self) -> usize {
        self.tool_result_sources
    }

    /// Returns the capability-relevant tool-choice mode.
    pub const fn choice(&self) -> ToolChoiceRequirement {
        self.choice
    }
}

/// Provider-private extension requirements retained outside portable semantic domains.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtensionRequirements {
    request: usize,
    input_items: usize,
    tool_inputs: usize,
    tool_declarations: usize,
    source_locations: usize,
}

impl ExtensionRequirements {
    /// Returns top-level request extension count.
    pub const fn request(&self) -> usize {
        self.request
    }

    /// Returns ordered input extension item count.
    pub const fn input_items(&self) -> usize {
        self.input_items
    }

    /// Returns Provider-private tool-call input count.
    pub const fn tool_inputs(&self) -> usize {
        self.tool_inputs
    }

    /// Returns Provider-private tool declaration count.
    pub const fn tool_declarations(&self) -> usize {
        self.tool_declarations
    }

    /// Returns Provider-private source-location count in tool results.
    pub const fn source_locations(&self) -> usize {
        self.source_locations
    }
}

/// Semantic output requirements projected from canonical output constraints.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutputRequirements {
    structured_output: StructuredOutputRequirement,
    structured_schema_bytes: Option<usize>,
    includes: BTreeSet<ResponseInclude>,
}

impl OutputRequirements {
    /// Returns the structured-output requirement.
    pub fn structured_output(&self) -> StructuredOutputRequirement {
        self.structured_output
    }

    /// Returns canonical compact-JSON schema bytes for schema-constrained output.
    pub const fn structured_schema_bytes(&self) -> Option<usize> {
        self.structured_schema_bytes
    }

    /// Returns requested additional response projections.
    pub fn includes(&self) -> &BTreeSet<ResponseInclude> {
        &self.includes
    }
}

/// Portable reasoning requirements projected from request controls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReasoningRequirements {
    request: ReasoningRequest,
    replay: bool,
}

impl ReasoningRequirements {
    /// Returns whether a reasoning object was absent or present.
    pub const fn presence(&self) -> ReasoningPresence {
        self.request.presence()
    }

    /// Returns the requested reasoning effort.
    pub const fn effort(&self) -> ReasoningEffort {
        self.request.effort()
    }

    /// Returns the requested reasoning summary mode.
    pub const fn summary(&self) -> ReasoningSummary {
        self.request.summary()
    }

    /// Returns whether ordered Provider reasoning replay is present.
    pub const fn replay(&self) -> bool {
        self.replay
    }
}

/// Portable generation-control requirements.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ControlRequirements {
    controls: GenerationControls,
}

impl ControlRequirements {
    /// Returns the requested output-token limit.
    pub const fn max_output_tokens(&self) -> Option<u64> {
        self.controls.max_output_tokens()
    }

    /// Returns the requested candidate count.
    pub const fn candidate_count(&self) -> Option<u32> {
        self.controls.candidate_count()
    }

    /// Returns explicit temperature, if supplied.
    pub fn temperature(&self) -> Option<f64> {
        self.controls.temperature()
    }

    /// Returns explicit top-p, if supplied.
    pub fn top_p(&self) -> Option<f64> {
        self.controls.top_p()
    }

    /// Returns explicit top-k, if supplied.
    pub const fn top_k(&self) -> Option<u64> {
        self.controls.top_k()
    }

    /// Returns stop sequences, preserving omission versus an explicit empty list.
    pub fn stop(&self) -> Option<&[super::TextValue]> {
        self.controls.stop()
    }

    /// Returns the explicit sampling seed.
    pub const fn seed(&self) -> Option<i64> {
        self.controls.seed()
    }

    /// Returns the explicit frequency penalty.
    pub fn frequency_penalty(&self) -> Option<f64> {
        self.controls.frequency_penalty()
    }

    /// Returns the explicit presence penalty.
    pub fn presence_penalty(&self) -> Option<f64> {
        self.controls.presence_penalty()
    }
}

/// Portable request-state requirements.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StateRequirements {
    continuation: bool,
    cache: bool,
    background: bool,
}

impl StateRequirements {
    /// Returns whether opaque continuation state is present.
    pub fn continuation(&self) -> bool {
        self.continuation
    }

    /// Returns whether prompt/cache state is requested.
    pub fn cache(&self) -> bool {
        self.cache
    }

    /// Returns whether background execution is requested.
    pub const fn background(&self) -> bool {
        self.background
    }
}

/// Registry-independent semantic facts projected from one canonical request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticRequirements {
    input: InputRequirements,
    tools: ToolRequirements,
    output: OutputRequirements,
    reasoning: ReasoningRequirements,
    controls: ControlRequirements,
    state: StateRequirements,
    extensions: ExtensionRequirements,
}

impl SemanticRequirements {
    /// Returns input semantic requirements.
    pub fn input(&self) -> &InputRequirements {
        &self.input
    }

    /// Returns tool semantic requirements.
    pub fn tools(&self) -> &ToolRequirements {
        &self.tools
    }

    /// Returns output semantic requirements.
    pub fn output(&self) -> &OutputRequirements {
        &self.output
    }

    /// Returns portable reasoning requirements.
    pub fn reasoning(&self) -> &ReasoningRequirements {
        &self.reasoning
    }

    /// Returns portable generation-control requirements.
    pub fn controls(&self) -> &ControlRequirements {
        &self.controls
    }

    /// Returns portable request-state requirements.
    pub fn state(&self) -> &StateRequirements {
        &self.state
    }

    /// Returns Provider-private extension requirements.
    pub fn extensions(&self) -> &ExtensionRequirements {
        &self.extensions
    }
}

fn record_resource(requirements: &mut InputRequirements, resource: &Resource) {
    let target = match resource.kind() {
        ResourceKind::Image => &mut requirements.image,
        ResourceKind::Audio => &mut requirements.audio,
        ResourceKind::File => &mut requirements.file,
    };
    target.count += 1;
    match resource.source() {
        ResourceSource::Url(url) => {
            target.url_sources += 1;
            target.max_url_bytes = target.max_url_bytes.max(url.as_str().len());
        }
        ResourceSource::Inline(bytes) => {
            target.inline_sources += 1;
            target.total_inline_bytes = target
                .total_inline_bytes
                .saturating_add(bytes.bytes().len());
        }
        ResourceSource::ProviderReference(_) => target.provider_references += 1,
    }
    if let Some(media_type) = resource.media_type() {
        target.media_types.insert(media_type.clone());
    }
    if let Some(detail) = resource.image_detail() {
        target.image_details.insert(detail);
    }
}

fn record_tool_result(requirements: &mut SemanticRequirements, result: &ToolResult) {
    requirements.tools.tool_history = true;
    for output in result.output() {
        match output {
            ToolOutput::Resource(resource) => record_resource(&mut requirements.input, resource),
            ToolOutput::Content(content) => {
                for part in content {
                    if let ContentPart::Resource(resource) = part {
                        record_resource(&mut requirements.input, resource);
                    }
                }
            }
            ToolOutput::Source(source) => {
                requirements.tools.tool_result_sources += 1;
                if matches!(source.location(), SourceLocation::Extension(_)) {
                    requirements.extensions.source_locations += 1;
                }
            }
            ToolOutput::Text(_) | ToolOutput::Json(_) => {}
        }
    }
}

/// Projects Registry-independent capability facts from a canonical request.
pub fn project_semantic_requirements(request: &GenerationRequest) -> SemanticRequirements {
    let mut requirements = SemanticRequirements::default();
    for item in request.input() {
        match item {
            InputItem::Instruction(_) => requirements.input.instructions = true,
            InputItem::Message(message) => {
                for part in message.content() {
                    match part {
                        ContentPart::Text(_) => requirements.input.text_parts += 1,
                        ContentPart::Resource(resource) => {
                            record_resource(&mut requirements.input, resource);
                        }
                    }
                }
            }
            InputItem::PriorToolCall(call) => {
                requirements.tools.tool_history = true;
                match call.input() {
                    ToolInput::Server(input) => {
                        requirements.tools.server_history.insert(input.kind());
                    }
                    ToolInput::Extension(_) => requirements.extensions.tool_inputs += 1,
                    ToolInput::Function(_) => {}
                }
            }
            InputItem::ToolResult(result) => record_tool_result(&mut requirements, result),
            InputItem::ReasoningReplay(_) => requirements.reasoning.replay = true,
            InputItem::Extension(_) => requirements.extensions.input_items += 1,
        }
    }
    for tool in request.tools() {
        if let ToolKind::Function(function) = tool.kind() {
            requirements.tools.function_tools = true;
            requirements.tools.function_tool_count += 1;
            requirements.tools.function_schema_bytes = requirements
                .tools
                .function_schema_bytes
                .saturating_add(function.parameters().encoded_len());
            requirements.tools.strict_function_tools |= function.strict();
        }
    }
    requirements.tools.server_tools = request
        .tools()
        .iter()
        .filter_map(|tool| match tool.kind() {
            ToolKind::Server(server) => Some(server.kind()),
            ToolKind::Function(_) | ToolKind::Extension(_) => None,
        })
        .collect();
    requirements.extensions.tool_declarations = request
        .tools()
        .iter()
        .filter(|tool| matches!(tool.kind(), ToolKind::Extension(_)))
        .count();
    requirements.tools.choice = request.tool_choice().requirement();
    requirements.tools.parallel_tool_calls = request.parallel_tool_calls();
    requirements.output.structured_output = match request.output() {
        OutputConstraint::Text => StructuredOutputRequirement::Text,
        OutputConstraint::JsonObject => StructuredOutputRequirement::JsonObject,
        OutputConstraint::JsonSchema { schema, strict, .. } => {
            requirements.output.structured_schema_bytes = Some(schema.encoded_len());
            StructuredOutputRequirement::JsonSchema { strict: *strict }
        }
    };
    requirements.output.includes = request.output_projection().includes().clone();
    requirements.reasoning.request = request.reasoning();
    requirements.controls.controls = request.controls().clone();
    requirements.state.continuation = request.state().continuation().is_some();
    requirements.state.cache = request.state().cache().is_some();
    requirements.state.background = request.state().background();
    requirements.extensions.request = request.extensions().len();
    requirements
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ir::generation::{
        BoundedOpaqueJson, CallId, ExtensionKind, ItemId, OpaquePayload, ParallelToolCalls,
        ProviderExtension, ProviderNamespace, ToolCall, ToolChoice, ToolDefinition, ToolExecutor,
        ToolInput, ToolKind, ToolName, ToolOrigin, ToolVisibility,
    };

    #[test]
    fn provider_extensions_project_at_each_owned_location() {
        let extension = ProviderExtension::new(
            ProviderNamespace::new("test", 64).expect("namespace must fit"),
            ExtensionKind::new("known-test-kind", 64).expect("kind must fit"),
            OpaquePayload::Json(
                BoundedOpaqueJson::new(json!({"value": 1}), 128).expect("payload must fit"),
            ),
            None,
        )
        .expect("originless downstream extension is valid");
        let request = GenerationRequest::new(vec![
            InputItem::Extension(extension.clone()),
            InputItem::PriorToolCall(ToolCall::new(
                ItemId::new("extension-item", 64).expect("item ID must fit"),
                CallId::new("extension-call", 64).expect("call ID must fit"),
                ToolName::new("provider_tool", 64).expect("tool name must fit"),
                ToolInput::Extension(extension.clone()),
                None,
            )),
        ])
        .expect("request must be valid")
        .with_extensions(vec![extension.clone()])
        .with_tools(
            vec![ToolDefinition::new(
                ToolName::new("provider_tool", 64).expect("tool name must fit"),
                ToolOrigin::Downstream,
                ToolExecutor::Client,
                ToolVisibility::Public,
                ToolKind::Extension(extension),
            )],
            ToolChoice::Auto,
            ParallelToolCalls::Inactive,
        )
        .expect("extension tool must be valid");

        let projected = project_semantic_requirements(&request);
        assert_eq!(projected.extensions().request(), 1);
        assert_eq!(projected.extensions().input_items(), 1);
        assert_eq!(projected.extensions().tool_inputs(), 1);
        assert_eq!(projected.extensions().tool_declarations(), 1);
    }
}
