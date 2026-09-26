//! Candidate-local representability over immutable final IR. No route or credential access.
use crate::{
    protocol::{
        fidelity::FidelityRecords,
        openai::{Profile, RequestRepresentation, ResponseMetadata, ResponseRepresentation},
    },
    semantic::task::generation::*,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationRepresentationContract {
    pub replay_origin: Option<crate::semantic::value::ReplayOrigin>,
    pub instructions: bool,
    pub temperature: bool,
    pub max_output_tokens: bool,
    pub tools: bool,
    pub custom_tools: bool,
    pub text_metadata: bool,
    pub top_p: bool,
    pub logprobs: bool,
    pub verbosity: bool,
    pub truncation: bool,
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
            replay_origin: None,
            instructions: true,
            temperature: true,
            max_output_tokens: true,
            tools: true,
            custom_tools: true,
            text_metadata: true,
            top_p: true,
            logprobs: true,
            verbosity: true,
            truncation: true,
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
    if q.custom_tools && !c.custom_tools {
        return Err(RepresentationError::Tools);
    }
    if q.text_metadata && !c.text_metadata {
        return Err(RepresentationError::TextMetadata);
    }
    if q.top_p && !c.top_p
        || q.logprobs && !c.logprobs
        || q.truncation && !c.truncation
        || !r.text_options().verbosity.is_absent() && !c.verbosity
    {
        return Err(RepresentationError::Controls);
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
    let q = check(r, c.clone())?;
    if profile == Profile::Chat
        && r.items()
            .iter()
            .any(|(_, i)| i.lifecycle().is_some_and(|s| s != ItemLifecycle::Completed))
    {
        return Err(RepresentationError::Terminal);
    }
    if profile == Profile::Chat
        && (q.logprobs || q.truncation || !r.text_options().verbosity.is_absent())
    {
        return Err(RepresentationError::UnmigratedSemantic);
    }
    if profile == Profile::Chat
        && r.items()
            .iter()
            .any(|(_, i)| matches!(i, Item::Message(m) if m.phase.is_some()))
    {
        // Phase labels are Responses-only; reject instead of dropping the label.
        return Err(RepresentationError::UnmigratedSemantic);
    }
    represent_reasoning(
        r.reasoning(),
        r.items(),
        fidelity,
        profile,
        c.replay_origin.as_ref(),
        true,
    )?;
    text_items(r.items(), profile)?;
    let expected_default = if profile == Profile::Chat {
        StrictDefault::NonStrict
    } else {
        StrictDefault::NormalizeSchema
    };
    for tool in r.tools() {
        if let ToolDefinition::Function(t) = tool {
            if matches!(t.strict, FunctionStrictness::Omitted(d) if d != expected_default) {
                return Err(RepresentationError::StrictDefault);
            }
            if profile == Profile::Chat && t.output_schema.is_some() {
                return Err(RepresentationError::Tools);
            }
        } else if profile == Profile::Chat {
            return Err(RepresentationError::Tools);
        }
    }
    if profile == Profile::Chat
        && matches!(
            r.tool_choice(),
            Some(ToolChoice::Custom(_) | ToolChoice::Allowed { .. })
        )
    {
        return Err(RepresentationError::Tools);
    }
    for (_, item) in r.items() {
        let parts: Vec<_> = match item {
            Item::Instruction(i) => i.parts.iter().map(|(id, _)| *id).collect(),
            Item::Message(m) => m.parts.iter().map(|p| p.id).collect(),
            Item::ToolResult(r) | Item::CustomResult(r) => match &r.output {
                ToolOutput::Parts(p) => p.iter().map(|(id, _)| *id).collect(),
                _ => vec![],
            },
            _ => vec![],
        };
        if profile == Profile::Chat && parts.iter().any(|id| fidelity.cache_breakpoint(*id)) {
            return Err(RepresentationError::Controls);
        }
        if let Item::Message(m)=item && m.parts.iter().any(|p|matches!(&p.content,ContentPart::Text(t) if !t.is_plain() && (m.role==MessageRole::User || fidelity.cache_breakpoint(p.id)))){return Err(RepresentationError::TextMetadata);}
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
    // Cache-write tokens are a reported Responses detail with no Chat wire projection.
    if profile == Profile::Chat
        && r.usage()
            .is_some_and(|usage| usage.input_cache_write_tokens.is_some())
    {
        return Err(RepresentationError::UnmigratedSemantic);
    }
    // Phase labels are Responses-only; reject instead of dropping the label.
    if profile == Profile::Chat
        && r.items()
            .iter()
            .any(|(_, i)| matches!(i, Item::Message(m) if m.phase.is_some()))
    {
        return Err(RepresentationError::UnmigratedSemantic);
    }
    // Reuse the same pure requirement projection for supported output items.
    if !r.items().is_empty() {
        let request = GenerationRequest::new(r.items().to_vec(), GenerationControls::default())?;
        check(&request, c.clone())?;
    }
    represent_reasoning(
        &ReasoningRequest::absent(),
        r.items(),
        fidelity,
        profile,
        c.replay_origin.as_ref(),
        false,
    )?;
    let chat_status = if r.outcome() == Outcome::Incomplete {
        ItemLifecycle::Incomplete
    } else {
        ItemLifecycle::Completed
    };
    if profile == Profile::Chat
        && r.items()
            .iter()
            .any(|(_, item)| item.lifecycle().is_some_and(|status| status != chat_status))
    {
        return Err(RepresentationError::Terminal);
    }
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
    if profile == Profile::Chat
        && (matches!(r.outcome(), Outcome::Failed | Outcome::Cancelled)
            || r.outcome() == Outcome::Incomplete
                && r.details().incomplete != Some(IncompleteReason::MaxOutputTokens)
            || r.details().error.is_some()
            || r.details()
                .incomplete
                .as_ref()
                .is_some_and(|reason| !matches!(reason, IncompleteReason::MaxOutputTokens)))
    {
        return Err(RepresentationError::Terminal);
    }
    if metadata.context.validate().is_err()
        || metadata
            .created
            .as_f64()
            .is_none_or(|v| !v.is_finite() || v < 0.0)
        || profile == Profile::Chat && metadata.created.as_u64().is_none()
    {
        return Err(RepresentationError::Metadata);
    }
    validate_wire_ids(r.items(), fidelity, true)?;
    Ok(ResponseRepresentation {
        semantic: r,
        fidelity,
        metadata,
        profile,
    })
}
fn represent_reasoning(
    controls: &ReasoningRequest,
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
    profile: Profile,
    origin: Option<&crate::semantic::value::ReplayOrigin>,
    request: bool,
) -> Result<(), RepresentationError> {
    for (id, item) in items {
        if let Item::Reasoning(reasoning) = item
            && let Some(replay) = fidelity.replay(*id)
            && (profile != Profile::Responses
                || !replay.permits(origin)
                || replay.value.replay_token().is_none()
                    && (request || reasoning.status == ItemLifecycle::Completed)
                || !fidelity.replay_matches(*id, reasoning))
        {
            return Err(RepresentationError::ReplayOrigin);
        }
    }
    let items = items
        .iter()
        .any(|(_, item)| matches!(item, Item::Reasoning(_)));
    if profile == Profile::Chat && (!controls.context.is_absent() || !controls.mode.is_absent()) {
        return Err(RepresentationError::Reasoning);
    }
    let chat_control = controls.presence() == ReasoningPresence::Present
        && (controls.effort().is_none() || controls.summary().is_some());
    if items && profile != Profile::Responses {
        return Err(RepresentationError::Reasoning);
    }
    if controls.encrypted_output() && profile != Profile::Responses {
        return Err(RepresentationError::Reasoning);
    }
    if profile == Profile::Chat && chat_control {
        return Err(RepresentationError::Reasoning);
    }
    Ok(())
}
fn text_items(items: &[(ItemId, Item)], profile: Profile) -> Result<(), RepresentationError> {
    for (_, i) in items {
        if profile == Profile::Chat {
            match i {
                Item::CustomCall(_) | Item::CustomResult(_) => {
                    return Err(RepresentationError::Tools);
                }
                Item::Instruction(i) if i.parts.len() != 1 => {
                    return Err(RepresentationError::MessageGrouping);
                }
                Item::ToolResult(r)
                    if !matches!(r.output, ToolOutput::Text(_))
                        || r.status.is_some_and(|s| s != ItemLifecycle::Completed) =>
                {
                    return Err(RepresentationError::Tools);
                }
                Item::Message(m)
                    if m.parts
                        .iter()
                        .any(|p| matches!(&p.content,ContentPart::Text(t) if !t.is_plain())) =>
                {
                    return Err(RepresentationError::TextMetadata);
                }
                _ => {}
            }
        }
        if let Item::Message(m) = i {
            if profile==Profile::Responses && m.parts.iter().any(|p|matches!(&p.content,ContentPart::Text(t) if t.logprobs().value().is_some_and(|v|v.iter().any(|p|p.bytes.is_none() || p.top_logprobs.as_ref().is_none_or(|v|v.iter().any(|p|p.token.is_none()||p.logprob.is_none()||p.bytes.is_none())))))){return Err(RepresentationError::TextMetadata);}
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
        if matches!(item, Item::Message(m) if m.parts.is_empty())
            && items
                .iter()
                .any(|(_, i)| matches!(i,Item::ToolCall(c) if c.message == Some(*id)))
        {
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
    #[error(transparent)]
    Event(#[from] EventError),
    #[error("target cannot represent text metadata")]
    TextMetadata,
    #[error("target cannot represent generation controls")]
    Controls,
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
    #[error("opaque replay requires a final token and a matching trusted origin")]
    ReplayOrigin,
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
