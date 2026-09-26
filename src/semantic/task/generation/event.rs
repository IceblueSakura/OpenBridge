//! Ordered Generation event state. Value, part, item and response closure are independent.
//! A failed reduction consumes the state; callers must not resume a rejected stream.
use super::*;
use crate::semantic::value::Text;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    Completed,
    Incomplete,
    Failed,
    Cancelled,
    Error,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Message {
        phase: Option<Phase>,
    },
    ToolCall {
        call_id: Text,
        name: Text,
        message: Option<ItemId>,
    },
    CustomCall {
        call_id: Text,
        name: Text,
    },
    Reasoning,
}
impl ItemKind {
    pub fn call(&self) -> Option<(&Text, &Text, Option<ItemId>)> {
        match self {
            Self::ToolCall {
                call_id,
                name,
                message,
            } => Some((call_id, name, *message)),
            Self::CustomCall { call_id, name } => Some((call_id, name, None)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartKind {
    Text,
    Refusal,
    Summary,
    ReasoningText,
    Arguments,
    CustomInput,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamEvent {
    Started,
    ItemStarted {
        item: ItemId,
        kind: ItemKind,
        replay: Option<ReasoningReplay>,
    },
    PartStarted {
        item: ItemId,
        part: PartId,
        kind: PartKind,
    },
    Delta {
        item: ItemId,
        part: PartId,
        fragment: String,
        logprobs: Vec<Logprob>,
    },
    LogprobsSnapshot {
        item: ItemId,
        part: PartId,
        logprobs: Vec<Logprob>,
    },
    AnnotationAdded {
        item: ItemId,
        part: PartId,
        annotation: Annotation,
    },
    TextMetadata {
        item: ItemId,
        part: PartId,
        annotations: Vec<Annotation>,
        logprobs: crate::semantic::value::Presence<Vec<Logprob>>,
    },
    ValueFinished {
        item: ItemId,
        part: PartId,
    },
    PartFinished {
        item: ItemId,
        part: PartId,
    },
    ItemFinished {
        item: ItemId,
        status: ItemLifecycle,
        replay: Option<ReasoningReplay>,
    },
    Usage(Usage),
    Terminal {
        terminal: StreamTerminal,
        details: TerminalDetails,
    },
}
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum EventError {
    #[error("conflicting event identity")]
    Identity,
    #[error("invalid event lifecycle")]
    Lifecycle,
    #[error("event state limit exceeded")]
    Limit,
    #[error("stream failed: {0:?}")]
    TerminalFailure(StreamTerminal),
    #[error("EOF before terminal")]
    EofBeforeTerminal,
    #[error(transparent)]
    Semantic(#[from] GenerationError),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamPart {
    pub id: PartId,
    pub kind: PartKind,
    pub text: String,
    pub value_finished: bool,
    pub finished: bool,
    pub annotations: Vec<Annotation>,
    pub logprobs: crate::semantic::value::Presence<Vec<Logprob>>,
    metadata_bytes: usize,
    metadata_finished: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamItem {
    pub id: ItemId,
    pub kind: ItemKind,
    pub parts: Vec<StreamPart>,
    pub status: Option<ItemLifecycle>,
    pub replay: Option<ReasoningReplay>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StreamState {
    started: bool,
    items: Vec<StreamItem>,
    part_ids: BTreeSet<PartId>,
    terminal: Option<StreamTerminal>,
    usage: Option<Usage>,
    details: TerminalDetails,
    bytes: usize,
}
impl StreamState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn items(&self) -> &[StreamItem] {
        &self.items
    }
    pub fn item(&self, id: ItemId) -> Result<&StreamItem, EventError> {
        self.items
            .iter()
            .find(|i| i.id == id)
            .ok_or(EventError::Identity)
    }
    pub fn part(&self, item: ItemId, part: PartId) -> Result<&StreamPart, EventError> {
        self.item(item)?
            .parts
            .iter()
            .find(|p| p.id == part)
            .ok_or(EventError::Identity)
    }
    pub fn terminal(&self) -> Option<StreamTerminal> {
        self.terminal
    }
    pub fn usage(&self) -> Option<Usage> {
        self.usage
    }
    pub fn details(&self) -> &TerminalDetails {
        &self.details
    }
    fn open_item(&mut self, id: ItemId) -> Result<&mut StreamItem, EventError> {
        self.items
            .iter_mut()
            .find(|i| i.id == id && i.status.is_none())
            .ok_or(EventError::Lifecycle)
    }
    fn open_part(&mut self, item: ItemId, part: PartId) -> Result<&mut StreamPart, EventError> {
        self.open_item(item)?
            .parts
            .iter_mut()
            .find(|p| p.id == part && !p.finished)
            .ok_or(EventError::Lifecycle)
    }
    fn charge(&mut self, n: usize) -> Result<(), EventError> {
        self.bytes = self.bytes.checked_add(n).ok_or(EventError::Limit)?;
        if self.bytes > MAX_TOTAL_BYTES {
            return Err(EventError::Limit);
        }
        Ok(())
    }
}
pub fn reduce(mut state: StreamState, event: StreamEvent) -> Result<StreamState, EventError> {
    if state.terminal.is_some() {
        return Err(EventError::Lifecycle);
    }
    if !state.started
        && !matches!(
            event,
            StreamEvent::Started
                | StreamEvent::Terminal {
                    terminal: StreamTerminal::Error,
                    ..
                }
        )
    {
        return Err(EventError::Lifecycle);
    }
    match event {
        StreamEvent::Started => {
            if state.started {
                return Err(EventError::Lifecycle);
            }
            state.started = true;
        }
        StreamEvent::ItemStarted { item, kind, replay } => {
            if state.items.len() >= MAX_ITEMS {
                return Err(EventError::Limit);
            }
            if state.items.iter().any(|i| i.id == item) {
                return Err(EventError::Identity);
            }
            if let Some((call_id, name, message)) = kind.call() {
                if call_id.as_str().is_empty()
                    || call_id.as_str().len() > 256
                    || name.as_str().is_empty()
                    || name.as_str().len() > 128
                {
                    return Err(EventError::Limit);
                }
                if state
                    .items
                    .iter()
                    .any(|i| i.kind.call().is_some_and(|(id, _, _)| id == call_id))
                {
                    return Err(EventError::Identity);
                }
                if let Some(owner) = message {
                    if !state
                        .items
                        .iter()
                        .any(|i| i.id == owner && matches!(i.kind, ItemKind::Message { .. }))
                    {
                        return Err(EventError::Identity);
                    }
                    if state.items.iter().rev().take_while(|i| i.id != owner).any(
                        |i| !matches!(&i.kind,ItemKind::ToolCall{message:Some(m),..} if *m==owner),
                    ) {
                        return Err(EventError::Lifecycle);
                    }
                }
                state.charge(call_id.as_str().len() + name.as_str().len())?;
            }
            if let Some(r) = &replay {
                r.validate()?;
                if !matches!(kind, ItemKind::Reasoning) || r.value.replay_token().is_some() {
                    return Err(EventError::Lifecycle);
                }
                state.charge(r.value.as_str().len())?;
            }
            state.items.push(StreamItem {
                id: item,
                kind,
                parts: vec![],
                status: None,
                replay,
            });
        }
        StreamEvent::PartStarted { item, part, kind } => {
            if state.part_ids.len() >= MAX_ITEMS {
                return Err(EventError::Limit);
            }
            if !state.part_ids.insert(part) {
                return Err(EventError::Identity);
            }
            let owner = state.open_item(item)?;
            let valid = matches!(
                (&owner.kind, kind),
                (ItemKind::Message { .. }, PartKind::Text | PartKind::Refusal)
                    | (
                        ItemKind::Reasoning,
                        PartKind::Summary | PartKind::ReasoningText
                    )
                    | (ItemKind::ToolCall { .. }, PartKind::Arguments)
                    | (ItemKind::CustomCall { .. }, PartKind::CustomInput)
            );
            if !valid
                || matches!(kind, PartKind::Arguments | PartKind::CustomInput)
                    && !owner.parts.is_empty()
            {
                return Err(EventError::Lifecycle);
            }
            owner.parts.push(StreamPart {
                id: part,
                kind,
                text: String::new(),
                value_finished: false,
                finished: false,
                annotations: vec![],
                logprobs: crate::semantic::value::Presence::Absent,
                metadata_bytes: 0,
                metadata_finished: false,
            });
        }
        StreamEvent::Delta {
            item,
            part,
            fragment,
            logprobs,
        } => {
            let target = state.open_part(item, part)?;
            if target.value_finished {
                return Err(EventError::Lifecycle);
            }
            if target
                .text
                .len()
                .saturating_add(fragment.len())
                .saturating_add(target.metadata_bytes)
                > MAX_TEXT_BYTES
            {
                return Err(EventError::Limit);
            }
            state.charge(fragment.len())?;
            state.open_part(item, part)?.text.push_str(&fragment);
            if !logprobs.is_empty() {
                if state.part(item, part)?.kind != PartKind::Text {
                    return Err(EventError::Lifecycle);
                }
                let mut probs = state
                    .part(item, part)?
                    .logprobs
                    .value()
                    .cloned()
                    .unwrap_or_default();
                probs.extend(logprobs);
                validate_logprobs(&probs)?;
                let bytes = metadata_cost(
                    &state.part(item, part)?.annotations,
                    &crate::semantic::value::Presence::Value(probs.clone()),
                )?;
                state.charge(bytes.saturating_sub(state.part(item, part)?.metadata_bytes))?;
                let p = state.open_part(item, part)?;
                if p.text.len().saturating_add(bytes) > MAX_TEXT_BYTES {
                    return Err(EventError::Limit);
                }
                p.metadata_bytes = bytes;
                p.logprobs = crate::semantic::value::Presence::Value(probs);
            }
        }
        StreamEvent::LogprobsSnapshot {
            item,
            part,
            logprobs,
        } => {
            let p = state.open_part(item, part)?;
            if p.kind != PartKind::Text
                || p.value_finished
                || !compatible_logprobs(
                    p.logprobs.value().map(Vec::as_slice).unwrap_or_default(),
                    &logprobs,
                )
            {
                return Err(EventError::Lifecycle);
            }
            validate_logprobs(&logprobs)?;
            let value = crate::semantic::value::Presence::Value(logprobs);
            let n = metadata_cost(&p.annotations, &value)?;
            let old = p.metadata_bytes;
            if p.text.len().saturating_add(n) > MAX_TEXT_BYTES {
                return Err(EventError::Limit);
            }
            state.bytes -= old;
            state.charge(n)?;
            let p = state.open_part(item, part)?;
            p.logprobs = value;
            p.metadata_bytes = n;
        }
        StreamEvent::AnnotationAdded {
            item,
            part,
            annotation,
        } => {
            let p = state.open_part(item, part)?;
            if p.kind != PartKind::Text || p.annotations.len() >= MAX_ITEMS {
                return Err(EventError::Lifecycle);
            }
            if p.metadata_finished {
                return Err(EventError::Lifecycle);
            }
            let mut annotations = p.annotations.clone();
            annotations.push(annotation);
            let n = metadata_cost(&annotations, &p.logprobs)?;
            let old = p.metadata_bytes;
            state.charge(n.saturating_sub(old))?;
            let p = state.open_part(item, part)?;
            if p.text.len().saturating_add(n) > MAX_TEXT_BYTES {
                return Err(EventError::Limit);
            }
            p.annotations = annotations;
            p.metadata_bytes = n;
        }
        StreamEvent::TextMetadata {
            item,
            part,
            annotations,
            logprobs,
        } => {
            let p = state.open_part(item, part)?;
            if p.kind != PartKind::Text
                || !p.value_finished
                || p.metadata_finished
                || !annotations.starts_with(&p.annotations)
            {
                return Err(EventError::Lifecycle);
            }
            let old = p.logprobs.value().cloned().unwrap_or_default();
            let next = logprobs.value().cloned().unwrap_or_default();
            if !compatible_logprobs(&old, &next) {
                return Err(EventError::Lifecycle);
            }
            let content = TextContent::new(
                Text::allowing_empty(&p.text, "text", MAX_TEXT_BYTES)
                    .map_err(|_| EventError::Limit)?,
                annotations.clone(),
                logprobs.clone(),
            )?;
            let new_bytes = metadata_cost(content.annotations(), content.logprobs())?;
            let old_bytes = p.metadata_bytes;
            state.bytes -= old_bytes;
            state.charge(new_bytes)?;
            let p = state.open_part(item, part)?;
            p.annotations = annotations;
            p.logprobs = logprobs;
            p.metadata_bytes = new_bytes;
            p.metadata_finished = true;
        }
        StreamEvent::ValueFinished { item, part } => {
            let p = state.open_part(item, part)?;
            if p.value_finished {
                return Err(EventError::Lifecycle);
            }
            p.value_finished = true;
        }
        StreamEvent::PartFinished { item, part } => {
            let p = state.open_part(item, part)?;
            if !p.value_finished {
                return Err(EventError::Lifecycle);
            }
            if p.kind == PartKind::Text {
                TextContent::new(
                    Text::allowing_empty(&p.text, "text", MAX_TEXT_BYTES)
                        .map_err(|_| EventError::Limit)?,
                    p.annotations.clone(),
                    p.logprobs.clone(),
                )?;
            }
            p.finished = true;
        }
        StreamEvent::ItemFinished {
            item,
            status,
            replay,
        } => {
            if status == ItemLifecycle::InProgress {
                return Err(EventError::Lifecycle);
            }
            let owner = state.open_item(item)?;
            if status == ItemLifecycle::Completed && owner.parts.iter().any(|p| !p.finished) {
                return Err(EventError::Lifecycle);
            }
            if owner.kind.call().is_some() && owner.parts.len() != 1 {
                return Err(EventError::Lifecycle);
            }
            if let Some(r) = &replay {
                r.validate()?;
                if !matches!(owner.kind, ItemKind::Reasoning) || r.value.replay_token().is_none() {
                    return Err(EventError::Lifecycle);
                }
                if owner
                    .replay
                    .as_ref()
                    .is_some_and(|old| old.origin != r.origin)
                {
                    return Err(EventError::Identity);
                }
            }
            let old = owner.replay.as_ref().map_or(0, |r| r.value.as_str().len());
            state.bytes -= old;
            state.charge(replay.as_ref().map_or(0, |r| r.value.as_str().len()))?;
            let owner = state.open_item(item)?;
            owner.replay = replay;
            owner.status = Some(status);
        }
        StreamEvent::Usage(usage) => {
            usage.validate()?;
            if state.usage.is_some() {
                return Err(EventError::Lifecycle);
            }
            state.usage = Some(usage);
        }
        StreamEvent::Terminal { terminal, details } => {
            if terminal == StreamTerminal::Completed
                && state
                    .items
                    .iter()
                    .any(|i| i.status != Some(ItemLifecycle::Completed))
            {
                return Err(EventError::Lifecycle);
            }
            let outcome = outcome(terminal, &state.items)?;
            details.validate(outcome)?;
            state.charge(details.bytes())?;
            state.details = details;
            state.terminal = Some(terminal);
        }
    }
    Ok(state)
}
fn metadata_cost(
    annotations: &[Annotation],
    probs: &crate::semantic::value::Presence<Vec<Logprob>>,
) -> Result<usize, EventError> {
    if annotations.is_empty() && probs.is_absent() {
        return Ok(0);
    }
    let a = crate::semantic::value::json_size(&annotations, MAX_TEXT_BYTES)
        .map_err(|_| EventError::Limit)?;
    let p = probs
        .value()
        .map(|p| {
            crate::semantic::value::json_size(p, MAX_TEXT_BYTES).map_err(|_| EventError::Limit)
        })
        .transpose()?
        .unwrap_or(0);
    let total = a.saturating_add(p);
    if total > MAX_TEXT_BYTES {
        return Err(EventError::Limit);
    }
    Ok(total)
}
fn outcome(terminal: StreamTerminal, items: &[StreamItem]) -> Result<Outcome, EventError> {
    Ok(match terminal {
        StreamTerminal::Completed => {
            Outcome::Completed(if items.iter().any(|i| i.kind.call().is_some()) {
                Completion::ToolCalls
            } else {
                Completion::Stop
            })
        }
        StreamTerminal::Incomplete => Outcome::Incomplete,
        StreamTerminal::Failed | StreamTerminal::Error => Outcome::Failed,
        StreamTerminal::Cancelled => Outcome::Cancelled,
    })
}
pub fn end_of_stream(state: &StreamState) -> Result<(), EventError> {
    if state.terminal.is_some() {
        Ok(())
    } else {
        Err(EventError::EofBeforeTerminal)
    }
}
/// Partial snapshots preserve item order and unfinished status; opaque replay stays in the sidecar.
pub fn snapshot_items(state: &StreamState) -> Result<Vec<(ItemId, Item)>, EventError> {
    state
        .items
        .iter()
        .map(|i| Ok((i.id, i.snapshot()?)))
        .collect()
}
impl StreamItem {
    pub(crate) fn snapshot(&self) -> Result<Item, EventError> {
        let i = self;
        let status = i.status.unwrap_or(ItemLifecycle::InProgress);
        let bounded = |p: &StreamPart| {
            Text::allowing_empty(&p.text, "event part", MAX_TEXT_BYTES)
                .map_err(|_| EventError::Limit)
        };
        let item = match &i.kind {
            ItemKind::Message { phase } => Item::Message(Message {
                role: MessageRole::Assistant,
                status,
                phase: *phase,
                parts: i
                    .parts
                    .iter()
                    .map(|p| {
                        Ok(Part {
                            id: p.id,
                            content: match p.kind {
                                PartKind::Text => ContentPart::Text(TextContent::new(
                                    bounded(p)?,
                                    p.annotations.clone(),
                                    p.logprobs.clone(),
                                )?),
                                PartKind::Refusal => ContentPart::Refusal(bounded(p)?),
                                _ => return Err(EventError::Lifecycle),
                            },
                        })
                    })
                    .collect::<Result<_, EventError>>()?,
            }),
            ItemKind::ToolCall {
                call_id,
                name,
                message,
            } => Item::ToolCall(ToolCall {
                call_id: call_id.clone(),
                name: name.clone(),
                message: *message,
                status,
                arguments: i.parts.first().map(|p| p.text.clone()).unwrap_or_default(),
            }),
            ItemKind::CustomCall { call_id, name } => Item::CustomCall(CustomCall {
                call_id: call_id.clone(),
                name: name.clone(),
                input: i.parts.first().map(|p| p.text.clone()).unwrap_or_default(),
            }),
            ItemKind::Reasoning => Item::Reasoning(ReasoningItem {
                status,
                parts: i
                    .parts
                    .iter()
                    .map(|p| {
                        Ok((
                            p.id,
                            match p.kind {
                                PartKind::Summary => ReasoningContent::Summary(bounded(p)?),
                                PartKind::ReasoningText => ReasoningContent::Text(bounded(p)?),
                                _ => return Err(EventError::Lifecycle),
                            },
                        ))
                    })
                    .collect::<Result<_, EventError>>()?,
            }),
        };
        Ok(item)
    }
}
pub fn materialize(state: &StreamState) -> Result<GenerationResponse, EventError> {
    let terminal = state.terminal.ok_or(EventError::EofBeforeTerminal)?;
    if terminal == StreamTerminal::Error {
        return Err(EventError::TerminalFailure(terminal));
    }
    let mut response =
        GenerationResponse::from_outcome(snapshot_items(state)?, outcome(terminal, &state.items)?)?
            .with_details(state.details.clone())?;
    if let Some(usage) = state.usage {
        response = response.with_usage(usage)?;
    }
    Ok(response)
}
