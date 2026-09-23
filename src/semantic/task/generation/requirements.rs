use super::{ContentPart, GenerationRequest, Item, OutputConstraint, ResourceKind};
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenerationRequirements {
    pub instruction_count: usize,
    pub message_count: usize,
    pub text_part_count: usize,
    pub resource_count: usize,
    pub image_inputs: usize,
    pub audio_inputs: usize,
    pub file_inputs: usize,
    pub tool_count: usize,
    pub custom_tools: bool,
    pub text_metadata: bool,
    pub top_p: bool,
    pub logprobs: bool,
    pub truncation: bool,
    pub tool_choice: Option<super::ToolChoice>,
    pub parallel_tool_calls: Option<bool>,
    pub strict_function_tools: bool,
    pub tool_history: bool,
    pub structured_output: bool,
    pub reasoning: bool,
    pub reasoning_items: usize,
    pub max_output_tokens: Option<u64>,
    pub temperature: bool,
}
impl GenerationRequirements {
    pub fn derive(r: &GenerationRequest) -> Self {
        let mut x = Self {
            tool_choice: r.tool_choice().cloned(),
            parallel_tool_calls: r.parallel_tool_calls(),
            instruction_count: usize::from(r.instructions().value().is_some()),
            custom_tools: r.tools().iter().any(|t|matches!(t,super::ToolDefinition::Custom(_))),
            top_p: r.controls().top_p().is_some(),
            logprobs: r.controls().logprobs || r.controls().top_logprobs.is_some(),
            truncation: r.controls().truncation.is_some(),
            strict_function_tools: r.tools().iter().any(|t| matches!(t,super::ToolDefinition::Function(t) if matches!(t.strict,super::FunctionStrictness::Explicit(true)|super::FunctionStrictness::Omitted(super::StrictDefault::NormalizeSchema)))),
            max_output_tokens: r.controls().max_output_tokens,
            temperature: r.controls().temperature().is_some(),
            tool_count: r.tools().len(),
            structured_output: !matches!(r.output(), OutputConstraint::Text),
            reasoning: r.reasoning().presence() == super::ReasoningPresence::Present
                || r.reasoning().encrypted_output(),
            ..Self::default()
        };
        for (_, i) in r.items() {
            match i {
                Item::Instruction(_) => x.instruction_count += 1,
                Item::Message(m) => {
                    // A tool-only Chat owner is grouping, not an extra semantic message.
                    x.message_count += usize::from(!m.parts.is_empty());
                    for p in &m.parts {
                        match &p.content {
                            ContentPart::Text(t) => {
                                x.text_part_count += 1;
                                x.text_metadata |= !t.is_plain();
                            }
                            ContentPart::Refusal(_) => {}
                            ContentPart::Resource(resource) => {
                                x.resource_count += 1;
                                match resource.kind {
                                    ResourceKind::Image => x.image_inputs += 1,
                                    ResourceKind::Audio => x.audio_inputs += 1,
                                    ResourceKind::File => x.file_inputs += 1,
                                }
                            }
                        }
                    }
                }
                Item::ToolCall(_) | Item::ToolResult(_) => x.tool_history = true,
                Item::CustomCall(_) | Item::CustomResult(_) => {
                    x.tool_history = true;
                    x.custom_tools = true;
                }
                Item::Reasoning(_) => {
                    x.reasoning_items += 1;
                    x.reasoning = true;
                }
            }
        }
        x
    }
}
