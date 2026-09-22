//! Function-call Event IR. Framing, usage, text, and other domains stay outside this slice.
use super::{
    Completion, GenerationError, GenerationResponse, Item, ItemId, MAX_ITEMS, MAX_TEXT_BYTES,
    MAX_TOTAL_BYTES, Message, MessageRole, Part, ToolCall,
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
    #[error("function-call event identity conflicts with the stream")]
    Identity,
    #[error("function-call event lifecycle is invalid")]
    Lifecycle,
    #[error("function-call event exceeds the slice limit")]
    Limit,
    #[error("non-completed terminal cannot materialize as success")]
    TerminalFailure(StreamTerminal),
    #[error("stream ended before a terminal")]
    EofBeforeTerminal,
    #[error(transparent)]
    Semantic(#[from] GenerationError),
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
    calls: Vec<Call>,
    terminal: Option<StreamTerminal>,
    bytes: usize,
}
impl StreamState {
    pub fn new() -> Self {
        Self {
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
        StreamEvent::CallStarted {
            item,
            call_id,
            name,
            message,
        } => {
            if state.calls.len() >= MAX_ITEMS
                || state.calls.iter().any(|call| call.item == item)
                || state.calls.iter().any(|call| call.call_id == call_id)
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
            let call = open_call(&mut state, item)?;
            call.finished = true;
        }
        StreamEvent::Terminal(terminal) => {
            if terminal == StreamTerminal::Completed
                && (state.calls.is_empty() || state.calls.iter().any(|call| !call.finished))
            {
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
    let mut items = Vec::new();
    if let Some(owner) = state.calls.first().and_then(|call| call.message) {
        items.push((
            owner,
            Item::Message(Message {
                role: MessageRole::Assistant,
                parts: Vec::<Part>::new(),
            }),
        ));
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
    Ok(GenerationResponse::new(items, Completion::ToolCalls)?)
}
fn open_call(state: &mut StreamState, item: ItemId) -> Result<&mut Call, EventError> {
    let call = state
        .calls
        .iter_mut()
        .find(|call| call.item == item)
        .ok_or(EventError::Identity)?;
    if call.finished {
        return Err(EventError::Lifecycle);
    }
    Ok(call)
}
fn add(total: &mut usize, value: &str) -> Result<(), EventError> {
    *total = total.checked_add(value.len()).ok_or(EventError::Limit)?;
    if *total > MAX_TOTAL_BYTES {
        return Err(EventError::Limit);
    }
    Ok(())
}
