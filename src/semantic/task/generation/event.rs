//! Function-call and assistant-text Event IR. Framing and other domains stay outside this slice.
use super::{
    Completion, ContentPart, GenerationError, GenerationResponse, Item, ItemId, MAX_ITEMS,
    MAX_TEXT_BYTES, MAX_TOTAL_BYTES, Message, MessageRole, Part, PartId, ToolCall,
};
use crate::semantic::value::Text;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    Completed,
    Failed,
    Incomplete,
    Error,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamEvent {
    TextStarted {
        item: ItemId,
        part: PartId,
    },
    TextDelta {
        item: ItemId,
        part: PartId,
        fragment: String,
    },
    TextFinished {
        item: ItemId,
        part: PartId,
    },
    CallStarted {
        item: ItemId,
        call_id: Text,
        name: Text,
        message: Option<ItemId>,
    },
    ArgumentsDelta {
        item: ItemId,
        fragment: String,
    },
    CallFinished {
        item: ItemId,
    },
    Terminal(StreamTerminal),
}
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum EventError {
    #[error("stream event identity conflicts with an existing item")]
    Identity,
    #[error("stream event lifecycle is invalid")]
    Lifecycle,
    #[error("stream event exceeds the slice limit")]
    Limit,
    #[error("non-completed terminal cannot materialize as success")]
    TerminalFailure(StreamTerminal),
    #[error("stream ended before a terminal")]
    EofBeforeTerminal,
    #[error(transparent)]
    Semantic(#[from] GenerationError),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct TextPart {
    item: ItemId,
    part: PartId,
    text: String,
    finished: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Call {
    item: ItemId,
    call_id: Text,
    name: Text,
    message: Option<ItemId>,
    arguments: String,
    finished: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamState {
    texts: Vec<TextPart>,
    calls: Vec<Call>,
    terminal: Option<StreamTerminal>,
    bytes: usize,
}
impl StreamState {
    pub fn new() -> Self {
        Self {
            texts: Vec::new(),
            calls: Vec::new(),
            terminal: None,
            bytes: 0,
        }
    }
    pub const fn terminal(&self) -> Option<StreamTerminal> {
        self.terminal
    }
}
impl Default for StreamState {
    fn default() -> Self {
        Self::new()
    }
}
pub fn reduce(mut state: StreamState, event: StreamEvent) -> Result<StreamState, EventError> {
    if state.terminal.is_some() {
        return Err(EventError::Lifecycle);
    }
    match event {
        StreamEvent::TextStarted { item, part } => {
            if state.texts.len() + state.calls.len() >= MAX_ITEMS
                || state.texts.iter().any(|text| text.part == part)
                || state.calls.iter().any(|call| call.item == item)
            {
                return Err(EventError::Identity);
            }
            if state
                .texts
                .iter()
                .any(|text| text.item == item && text.finished)
            {
                return Err(EventError::Lifecycle);
            }
            state.texts.push(TextPart {
                item,
                part,
                text: String::new(),
                finished: false,
            });
        }
        StreamEvent::TextDelta {
            item,
            part,
            fragment,
        } => {
            if fragment.is_empty() {
                return Err(EventError::Lifecycle);
            }
            let current = open_text(&state, item, part)?.text.len();
            if current + fragment.len() > MAX_TEXT_BYTES {
                return Err(EventError::Limit);
            }
            add(&mut state.bytes, &fragment)?;
            open_text_mut(&mut state, item, part)?
                .text
                .push_str(&fragment);
        }
        StreamEvent::TextFinished { item, part } => {
            let text = open_text_mut(&mut state, item, part)?;
            if text.text.is_empty() {
                return Err(EventError::Lifecycle);
            }
            text.finished = true;
        }
        StreamEvent::CallStarted {
            item,
            call_id,
            name,
            message,
        } => {
            if state.calls.len() >= MAX_ITEMS
                || state.calls.iter().any(|call| call.item == item)
                || state.calls.iter().any(|call| call.call_id == call_id)
                || state.texts.iter().any(|text| text.item == item)
                || call_id.as_str().len() > 256
                || name.as_str().len() > 128
            {
                return Err(EventError::Identity);
            }
            if state
                .calls
                .first()
                .is_some_and(|call| call.message != message)
            {
                return Err(EventError::Lifecycle);
            }
            add(&mut state.bytes, call_id.as_str())?;
            add(&mut state.bytes, name.as_str())?;
            state.calls.push(Call {
                item,
                call_id,
                name,
                message,
                arguments: String::new(),
                finished: false,
            });
        }
        StreamEvent::ArgumentsDelta { item, fragment } => {
            if fragment.is_empty() {
                return Err(EventError::Lifecycle);
            }
            let current = state
                .calls
                .iter()
                .find(|call| call.item == item && !call.finished)
                .ok_or(if state.calls.iter().any(|call| call.item == item) {
                    EventError::Lifecycle
                } else {
                    EventError::Identity
                })?
                .arguments
                .len();
            if current + fragment.len() > MAX_TEXT_BYTES {
                return Err(EventError::Limit);
            }
            add(&mut state.bytes, &fragment)?;
            open_call(&mut state, item)?.arguments.push_str(&fragment);
        }
        StreamEvent::CallFinished { item } => {
            open_call(&mut state, item)?.finished = true;
        }
        StreamEvent::Terminal(terminal) => {
            let open = state.texts.iter().any(|text| !text.finished)
                || state.calls.iter().any(|call| !call.finished);
            let empty = state.texts.is_empty() && state.calls.is_empty();
            if terminal == StreamTerminal::Completed && (empty || open) {
                return Err(EventError::Lifecycle);
            }
            state.terminal = Some(terminal);
        }
    }
    Ok(state)
}
pub fn end_of_stream(state: &StreamState) -> Result<(), EventError> {
    if state.terminal.is_none() {
        Err(EventError::EofBeforeTerminal)
    } else {
        Ok(())
    }
}
pub fn materialize(state: &StreamState) -> Result<GenerationResponse, EventError> {
    match state.terminal {
        Some(StreamTerminal::Completed) => {}
        Some(terminal) => return Err(EventError::TerminalFailure(terminal)),
        None => return Err(EventError::Lifecycle),
    }
    let completion = if state.calls.is_empty() {
        Completion::Stop
    } else {
        Completion::ToolCalls
    };
    let mut items = Vec::new();
    let owner = state.calls.first().and_then(|call| call.message);
    if let Some(owner) = owner {
        items.push((owner, message_item(state, owner)?));
    } else {
        let mut seen = Vec::new();
        for text in &state.texts {
            if !seen.contains(&text.item) {
                seen.push(text.item);
                items.push((text.item, message_item(state, text.item)?));
            }
        }
    }
    for call in &state.calls {
        items.push((
            call.item,
            Item::ToolCall(ToolCall {
                call_id: call.call_id.clone(),
                name: call.name.clone(),
                arguments: call.arguments.clone(),
                message: call.message,
            }),
        ));
    }
    Ok(GenerationResponse::new(items, completion)?)
}
fn message_item(state: &StreamState, item: ItemId) -> Result<Item, EventError> {
    let parts = state
        .texts
        .iter()
        .filter(|text| text.item == item)
        .map(|text| {
            Ok(Part {
                id: text.part,
                content: ContentPart::Text(
                    Text::allowing_empty(text.text.clone(), "text", MAX_TEXT_BYTES)
                        .map_err(|_| EventError::Limit)?,
                ),
            })
        })
        .collect::<Result<Vec<_>, EventError>>()?;
    Ok(Item::Message(Message {
        role: MessageRole::Assistant,
        parts,
    }))
}
fn open_text(state: &StreamState, item: ItemId, part: PartId) -> Result<&TextPart, EventError> {
    state
        .texts
        .iter()
        .find(|text| text.item == item && text.part == part && !text.finished)
        .ok_or(if state.texts.iter().any(|text| text.part == part) {
            EventError::Lifecycle
        } else {
            EventError::Identity
        })
}
fn open_text_mut(
    state: &mut StreamState,
    item: ItemId,
    part: PartId,
) -> Result<&mut TextPart, EventError> {
    let finished = state
        .texts
        .iter()
        .any(|text| text.part == part && text.finished);
    state
        .texts
        .iter_mut()
        .find(|text| text.item == item && text.part == part && !text.finished)
        .ok_or(if finished {
            EventError::Lifecycle
        } else {
            EventError::Identity
        })
}
fn open_call(state: &mut StreamState, item: ItemId) -> Result<&mut Call, EventError> {
    let missing = !state.calls.iter().any(|call| call.item == item);
    state
        .calls
        .iter_mut()
        .find(|call| call.item == item && !call.finished)
        .ok_or(if missing {
            EventError::Identity
        } else {
            EventError::Lifecycle
        })
}
fn add(total: &mut usize, value: &str) -> Result<(), EventError> {
    *total = total.checked_add(value.len()).ok_or(EventError::Limit)?;
    if *total > MAX_TOTAL_BYTES {
        return Err(EventError::Limit);
    }
    Ok(())
}
