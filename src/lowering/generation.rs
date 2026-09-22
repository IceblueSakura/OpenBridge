//! Candidate-local representability over immutable final IR. No route or credential access.
use crate::{
    protocol::{
        fidelity::FidelityRecords,
        openai::{Profile, RequestRepresentation, ResponseMetadata, ResponseRepresentation},
    },
    semantic::task::generation::*,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationRepresentationContract {
    pub instructions: bool,
    pub temperature: bool,
    pub max_output_tokens: bool,
    pub tools: bool,
    pub structured_output: bool,
    pub reasoning: bool,
    pub image_input: bool,
    pub audio_input: bool,
    pub file_input: bool,
    pub parallel_tool_calls: bool,
    pub strict_tools: bool,
}
impl GenerationRepresentationContract {
    pub const fn full() -> Self {
        Self {
            instructions: true,
            temperature: true,
            max_output_tokens: true,
            tools: true,
            structured_output: true,
            reasoning: true,
            image_input: true,
            audio_input: true,
            file_input: true,
            parallel_tool_calls: true,
            strict_tools: true,
        }
    }
}
pub fn check(
    r: &GenerationRequest,
    c: GenerationRepresentationContract,
) -> Result<GenerationRequirements, RepresentationError> {
    r.validate()?;
    let q = GenerationRequirements::derive(r);
    if q.instruction_count > 0 && !c.instructions {
        return Err(RepresentationError::Instructions);
    }
    if q.temperature && !c.temperature {
        return Err(RepresentationError::Temperature);
    }
    if q.max_output_tokens.is_some() && !c.max_output_tokens {
        return Err(RepresentationError::MaxOutputTokens);
    }
    if (q.tool_count > 0
        || q.tool_history
        || q.tool_choice.is_some()
        || q.parallel_tool_calls.is_some())
        && !c.tools
    {
        return Err(RepresentationError::Tools);
    }
    if q.parallel_tool_calls == Some(true) && !c.parallel_tool_calls {
        return Err(RepresentationError::ParallelTools);
    }
    if q.strict_function_tools && !c.strict_tools {
        return Err(RepresentationError::StrictTools);
    }
    if q.structured_output && !c.structured_output {
        return Err(RepresentationError::StructuredOutput);
    }
    if q.reasoning && !c.reasoning {
        return Err(RepresentationError::Reasoning);
    }
    if q.image_inputs > 0 && !c.image_input {
        return Err(RepresentationError::ImageInput);
    }
    if q.audio_inputs > 0 && !c.audio_input {
        return Err(RepresentationError::AudioInput);
    }
    if q.file_inputs > 0 && !c.file_input {
        return Err(RepresentationError::FileInput);
    }
    Ok(q)
}
pub fn lower_request<'a>(
    r: &'a GenerationRequest,
    fidelity: &'a FidelityRecords,
    profile: Profile,
    c: GenerationRepresentationContract,
) -> Result<RequestRepresentation<'a>, RepresentationError> {
    let q = check(r, c)?;
    if q.structured_output || q.reasoning {
        return Err(RepresentationError::UnmigratedSemantic);
    }
    text_items(r.items(), profile)?;
    let expected_default = if profile == Profile::Chat {
        StrictDefault::NonStrict
    } else {
        StrictDefault::NormalizeSchema
    };
    for ToolDefinition::Function(t) in r.tools() {
        if matches!(t.strict, FunctionStrictness::Omitted(d) if d != expected_default) {
            return Err(RepresentationError::StrictDefault);
        }
    }
    validate_wire_ids(r.items(), fidelity, false)?;
    Ok(RequestRepresentation {
        semantic: r,
        fidelity,
        profile,
    })
}
pub fn lower_response<'a>(
    r: &'a GenerationResponse,
    fidelity: &'a FidelityRecords,
    metadata: &'a ResponseMetadata,
    profile: Profile,
    c: GenerationRepresentationContract,
) -> Result<ResponseRepresentation<'a>, RepresentationError> {
    // Reuse the same pure requirement projection for supported output items.
    let request = GenerationRequest::new(r.items().to_vec(), GenerationControls::default())?;
    check(&request, c)?;
    text_items(r.items(), profile)?;
    if profile == Profile::Chat && chat_message_count(r.items()) != 1 {
        return Err(RepresentationError::MessageGrouping);
    }
    if metadata.id.is_empty()
        || metadata.model.is_empty()
        || metadata.id.len() > 256
        || metadata.model.len() > 256
    {
        return Err(RepresentationError::Metadata);
    }
    if profile == Profile::Chat && matches!(r.outcome(), Outcome::Failed) {
        return Err(RepresentationError::Terminal);
    }
    validate_wire_ids(r.items(), fidelity, true)?;
    Ok(ResponseRepresentation {
        semantic: r,
        fidelity,
        metadata,
        profile,
    })
}
fn text_items(items: &[(ItemId, Item)], profile: Profile) -> Result<(), RepresentationError> {
    for (_, i) in items {
        if let Item::Message(m) = i {
            if m.parts
                .iter()
                .any(|p| !matches!(p.content, ContentPart::Text(_) | ContentPart::Refusal(_)))
            {
                return Err(RepresentationError::UnmigratedSemantic);
            }
            if profile == Profile::Chat && m.parts.len() > 1 {
                return Err(RepresentationError::MessageGrouping);
            }
        }
    }
    Ok(())
}
/// Explicit normalization: a contiguous run of independent function calls is one Chat call message.
/// Independent text messages are never merged into that run by positional guessing.
fn chat_message_count(items: &[(ItemId, Item)]) -> usize {
    let mut count = 0;
    let mut run = false;
    for (_, item) in items {
        match item {
            Item::ToolCall(c) if c.message.is_none() => {
                if !run {
                    count += 1;
                }
                run = true;
            }
            Item::ToolCall(_) => run = false,
            _ => {
                count += 1;
                run = false;
            }
        }
    }
    count
}
fn validate_wire_ids(
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
    generate: bool,
) -> Result<(), RepresentationError> {
    let mut ids = BTreeSet::new();
    for (id, item) in items {
        if matches!(item, Item::Message(m) if m.parts.is_empty()) {
            continue;
        }
        let value = fidelity
            .response_item_id(*id)
            .map(str::to_owned)
            .or_else(|| generate.then(|| format!("item_{}", id.get())));
        if value.is_some_and(|value| !ids.insert(value)) {
            return Err(RepresentationError::Metadata);
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RepresentationError {
    #[error(transparent)]
    Semantic(#[from] GenerationError),
    #[error("target cannot represent instructions")]
    Instructions,
    #[error("target cannot represent temperature")]
    Temperature,
    #[error("target cannot represent maximum output tokens")]
    MaxOutputTokens,
    #[error("target cannot represent tools")]
    Tools,
    #[error("target cannot represent parallel tools")]
    ParallelTools,
    #[error("target cannot represent strict tools")]
    StrictTools,
    #[error("omitted strict has a different default in the target profile")]
    StrictDefault,
    #[error("target cannot represent structured output")]
    StructuredOutput,
    #[error("target cannot represent reasoning")]
    Reasoning,
    #[error("target cannot represent image input")]
    ImageInput,
    #[error("target cannot represent audio input")]
    AudioInput,
    #[error("target cannot represent file input")]
    FileInput,
    #[error("semantic domain has no migrated codec yet")]
    UnmigratedSemantic,
    #[error("message grouping cannot be preserved in this target")]
    MessageGrouping,
    #[error("terminal cannot be represented by the target profile")]
    Terminal,
    #[error("usage conversion is not part of this migration slice")]
    UsageProjection,
    #[error("invalid or conflicting response representation metadata")]
    Metadata,
}
