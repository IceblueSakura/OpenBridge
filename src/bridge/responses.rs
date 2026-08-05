//! Bridge lifecycle state machine for Responses SSE events.
//!
//! This module fixes response, output-item, and function-call identities, accumulates text and
//! arguments item by item, and requires every output item to be explicitly complete before the
//! completed terminal.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::transport::sse::SseEvent;

use super::{
    BridgeStreamError, BridgeToolCall, Lifecycle, StreamTerminal,
    shared::{required_str, required_u64, validate_arguments},
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ResponsesItem {
    Message {
        item_id: String,
        text: String,
        completed: bool,
    },
    Reasoning {
        item_id: String,
        text: String,
        completed: bool,
    },
    Tool(BridgeToolCall),
}

/// Per-request state that reconstructs Bridge semantics from the Responses SSE lifecycle.
#[derive(Debug)]
pub struct ResponsesStreamState {
    lifecycle: Lifecycle,
    response_id: Option<String>,
    items: BTreeMap<u64, ResponsesItem>,
}

impl ResponsesStreamState {
    /// Creates an empty state machine waiting for `response.created`.
    pub fn new() -> Self {
        Self {
            lifecycle: Lifecycle::AwaitingStart,
            response_id: None,
            items: BTreeMap::new(),
        }
    }

    /// Consumes one fully framed Responses SSE event.
    pub fn ingest(&mut self, event: &SseEvent) -> Result<(), BridgeStreamError> {
        // Parse the JSON and validate the SSE event name and payload type together.
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeStreamError::InvalidJson)?;
        let payload_type = required_str(&value, "type")?;
        if event.event().is_some_and(|name| name != payload_type) {
            return Err(BridgeStreamError::EventTypeConflict);
        }

        // Advance lifecycle and output-item state from the explicit event type.
        match payload_type {
            "response.created" => self.on_created(&value),
            "response.output_item.added" => self.on_item_added(&value),
            "response.output_text.delta" => self.on_text_delta(&value),
            "response.reasoning_summary_part.added" => self.on_reasoning_part_added(&value),
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                self.on_reasoning_delta(&value)
            }
            "response.reasoning_summary_text.done" | "response.reasoning_text.done" => {
                self.on_reasoning_done(&value)
            }
            "response.function_call_arguments.delta" => self.on_arguments_delta(&value),
            "response.function_call_arguments.done" => self.on_arguments_done(&value),
            "response.output_item.done" => self.on_item_done(&value),
            "response.completed" => self.on_terminal(&value, StreamTerminal::Completed),
            "response.failed" => self.on_terminal(&value, StreamTerminal::Failed),
            "response.incomplete" => self.on_terminal(&value, StreamTerminal::Incomplete),
            "error" => self.on_error(),
            _ => Err(BridgeStreamError::UnexpectedEvent),
        }
    }

    /// Verifies that the single terminal appeared when the upstream reaches EOF.
    pub fn finish(&self) -> Result<(), BridgeStreamError> {
        match self.lifecycle {
            Lifecycle::Terminal(_) => Ok(()),
            Lifecycle::AwaitingStart | Lifecycle::Streaming => {
                Err(BridgeStreamError::EofBeforeTerminal)
            }
        }
    }

    /// Returns the confirmed terminal state.
    pub fn terminal(&self) -> Option<StreamTerminal> {
        match self.lifecycle {
            Lifecycle::Terminal(terminal) => Some(terminal),
            Lifecycle::AwaitingStart | Lifecycle::Streaming => None,
        }
    }

    /// Returns assistant text concatenated by output index.
    pub fn text(&self) -> String {
        self.items
            .values()
            .filter_map(|item| match item {
                ResponsesItem::Message { text, .. } => Some(text.as_str()),
                ResponsesItem::Reasoning { .. } | ResponsesItem::Tool(_) => None,
            })
            .collect()
    }

    /// Returns plain reasoning text concatenated by output index.
    pub fn reasoning_text(&self) -> String {
        self.items
            .values()
            .filter_map(|item| match item {
                ResponsesItem::Reasoning { text, .. } => Some(text.as_str()),
                ResponsesItem::Message { .. } | ResponsesItem::Tool(_) => None,
            })
            .collect()
    }

    /// Returns function tool calls ordered by output index.
    pub fn tool_calls(&self) -> Vec<&BridgeToolCall> {
        self.items
            .values()
            .filter_map(|item| match item {
                ResponsesItem::Tool(tool) => Some(tool),
                ResponsesItem::Message { .. } | ResponsesItem::Reasoning { .. } => None,
            })
            .collect()
    }

    /// Accepts the single `response.created` event and fixes the response identity.
    fn on_created(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Allow only one response creation and fix the response identity.
        if self.lifecycle != Lifecycle::AwaitingStart {
            return Err(BridgeStreamError::UnexpectedEvent);
        }
        let response = value
            .get("response")
            .ok_or(BridgeStreamError::InvalidJson)?;
        self.response_id = Some(required_str(response, "id")?.to_owned());
        self.lifecycle = Lifecycle::Streaming;
        Ok(())
    }

    /// Registers an output item with a unique index, item ID, and call ID.
    fn on_item_added(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Verify that the stream has started and extract the stable output identity.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        if self.items.contains_key(&output_index) {
            return Err(BridgeStreamError::DuplicateIdentity);
        }
        let item = value.get("item").ok_or(BridgeStreamError::InvalidJson)?;
        let item_id = required_str(item, "id")?.to_owned();
        if self.items.values().any(|known| match known {
            ResponsesItem::Message { item_id: known, .. } => known == &item_id,
            ResponsesItem::Reasoning { item_id: known, .. } => known == &item_id,
            ResponsesItem::Tool(tool) => tool.item_id.as_deref() == Some(item_id.as_str()),
        }) {
            return Err(BridgeStreamError::DuplicateIdentity);
        }

        // Initialize an independent text or tool-call accumulator for the item type.
        let parsed = match required_str(item, "type")? {
            "message" => ResponsesItem::Message {
                item_id,
                text: String::new(),
                completed: false,
            },
            "reasoning" => {
                reject_encrypted_reasoning(item)?;
                ResponsesItem::Reasoning {
                    item_id,
                    text: reasoning_item_text(item)?,
                    completed: false,
                }
            }
            "function_call" => {
                let call_id = required_str(item, "call_id")?.to_owned();
                if self.items.values().any(
                    |known| matches!(known, ResponsesItem::Tool(tool) if tool.call_id == call_id),
                ) {
                    return Err(BridgeStreamError::DuplicateIdentity);
                }
                ResponsesItem::Tool(BridgeToolCall {
                    output_index,
                    item_id: Some(item_id),
                    call_id,
                    name: required_str(item, "name")?.to_owned(),
                    arguments: item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    completed: false,
                })
            }
            _ => return Err(BridgeStreamError::UnexpectedEvent),
        };
        self.items.insert(output_index, parsed);
        Ok(())
    }

    /// Binds a text delta to a registered, incomplete message item.
    fn on_text_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Bind the delta to the registered message item instead of inferring identity from an index.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let delta = required_str(value, "delta")?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Message {
                     item_id: known,
                     text,
                     completed: false,
                 }) if known == item_id => {
                text.push_str(delta);
                Ok(())
            }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Binds a reasoning summary/text delta to a registered reasoning item.
    fn on_reasoning_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Accumulate reasoning separately from visible text; do not reuse the Chat message text accumulator.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let delta = required_str(value, "delta")?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Reasoning {
                     item_id: known,
                     text,
                     completed: false,
                 }) if known == item_id => {
                text.push_str(delta);
                Ok(())
            }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Validates the reasoning summary-part identity and initial text.
    fn on_reasoning_part_added(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // The Responses summary part still belongs to the same reasoning output item.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let part = value
            .get("part")
            .and_then(Value::as_object)
            .ok_or(BridgeStreamError::InvalidJson)?;
        if part.get("type").and_then(Value::as_str) != Some("summary_text") {
            return Err(BridgeStreamError::UnexpectedEvent);
        }
        let text = part
            .get("text")
            .and_then(Value::as_str)
            .ok_or(BridgeStreamError::InvalidJson)?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Reasoning {
                     item_id: known,
                     text: accumulated,
                     completed: false,
                 }) if known == item_id => {
                accumulated.push_str(text);
                Ok(())
            }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Validates a reasoning summary/text completion snapshot against accumulated values.
    fn on_reasoning_done(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // The done event must exactly match the reasoning deltas already received.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let text = required_str(value, "text")?;
        match self.items.get(&output_index) {
            Some(ResponsesItem::Reasoning {
                     item_id: known,
                     text: accumulated,
                     completed: false,
                 }) if known == item_id && accumulated == text => Ok(()),
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Binds an arguments delta to a registered, incomplete function-call item.
    fn on_arguments_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Bind argument fragments to the stable item ID and reject appends after completion.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let delta = required_str(value, "delta")?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Tool(tool))
            if tool.item_id.as_deref() == Some(item_id) && !tool.completed =>
                {
                    tool.arguments.push_str(delta);
                    Ok(())
                }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Validates the complete function-arguments snapshot against accumulated values.
    fn on_arguments_done(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Verify that the upstream done snapshot matches the accumulated arguments and forms a JSON object.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let arguments = required_str(value, "arguments")?;
        match self.items.get(&output_index) {
            Some(ResponsesItem::Tool(tool))
            if tool.item_id.as_deref() == Some(item_id) && tool.arguments == arguments =>
                {
                    validate_arguments(arguments)
                }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Validates and completes the specified output item against its full snapshot.
    fn on_item_done(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Validate identity and accumulated content against the full item snapshot, then mark it complete.
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let snapshot = value.get("item").ok_or(BridgeStreamError::InvalidJson)?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Message {
                     item_id,
                     text,
                     completed,
                 }) => {
                if *completed || required_str(snapshot, "id")? != item_id {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                let snapshot_text = snapshot
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| content.iter().find_map(|part| part.get("text")))
                    .and_then(Value::as_str)
                    .ok_or(BridgeStreamError::InvalidJson)?;
                if snapshot_text != text {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                *completed = true;
                Ok(())
            }
            Some(ResponsesItem::Reasoning {
                     item_id,
                     text,
                     completed,
                 }) => {
                if *completed || required_str(snapshot, "id")? != item_id {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                if reasoning_item_text(snapshot)? != *text {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                *completed = true;
                Ok(())
            }
            Some(ResponsesItem::Tool(tool)) => {
                if tool.completed
                    || required_str(snapshot, "id")? != tool.item_id.as_deref().unwrap_or_default()
                    || required_str(snapshot, "call_id")? != tool.call_id
                    || required_str(snapshot, "name")? != tool.name
                    || required_str(snapshot, "arguments")? != tool.arguments
                {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                validate_arguments(&tool.arguments)?;
                tool.completed = true;
                Ok(())
            }
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// Accepts an explicit Responses terminal and validates response identity and completion boundaries.
    fn on_terminal(
        &mut self,
        value: &Value,
        terminal: StreamTerminal,
    ) -> Result<(), BridgeStreamError> {
        // Reject a duplicate terminal and verify that its response identity has not changed.
        if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
            return Err(BridgeStreamError::DuplicateTerminal);
        }
        self.ensure_streaming()?;
        let response = value
            .get("response")
            .ok_or(BridgeStreamError::InvalidJson)?;
        if required_str(response, "id")? != self.response_id.as_deref().unwrap_or_default() {
            return Err(BridgeStreamError::IdentityConflict);
        }

        // A successful terminal must wait for every output item to complete; failure states never masquerade as completion.
        if terminal == StreamTerminal::Completed
            && self.items.values().any(|item| match item {
            ResponsesItem::Message { completed, .. } => !completed,
            ResponsesItem::Reasoning { completed, .. } => !completed,
            ResponsesItem::Tool(tool) => !tool.completed,
        })
        {
            return Err(BridgeStreamError::IncompleteOutputItem);
        }
        self.lifecycle = Lifecycle::Terminal(terminal);
        Ok(())
    }

    /// Accepts an independent `error` event and fixes it as the current response's failure terminal.
    fn on_error(&mut self) -> Result<(), BridgeStreamError> {
        // Reject an independent error event after an existing terminal.
        if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
            return Err(BridgeStreamError::DuplicateTerminal);
        }

        // An independent error can terminate the stream only after the response is created.
        self.ensure_streaming()?;
        self.lifecycle = Lifecycle::Terminal(StreamTerminal::Error);
        Ok(())
    }

    /// Confirms that the current state allows a normal Responses output event.
    fn ensure_streaming(&self) -> Result<(), BridgeStreamError> {
        match self.lifecycle {
            Lifecycle::Streaming => Ok(()),
            Lifecycle::AwaitingStart | Lifecycle::Terminal(_) => {
                Err(BridgeStreamError::UnexpectedEvent)
            }
        }
    }
}

impl Default for ResponsesStreamState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts plain content and summary from a reasoning item and rejects opaque continuation.
fn reasoning_item_text(item: &Value) -> Result<String, BridgeStreamError> {
    reject_encrypted_reasoning(item)?;
    let mut text = String::new();
    for (field, expected_type) in [("content", "reasoning_text"), ("summary", "summary_text")] {
        let Some(parts) = item.get(field).and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            let part = part.as_object().ok_or(BridgeStreamError::InvalidJson)?;
            if part.get("type").and_then(Value::as_str) != Some(expected_type) {
                return Err(BridgeStreamError::UnexpectedEvent);
            }
            text.push_str(required_str(&Value::Object(part.clone()), "text")?);
        }
    }
    Ok(text)
}

/// Rejects reasoning items whose opaque Responses continuation cannot become plain Chat text.
fn reject_encrypted_reasoning(item: &Value) -> Result<(), BridgeStreamError> {
    let Some(value) = item
        .get("encrypted_content")
        .filter(|value| !value.is_null())
    else {
        return Ok(());
    };
    let content = value.as_str().ok_or(BridgeStreamError::InvalidJson)?;
    if content.is_empty() {
        Ok(())
    } else {
        Err(BridgeStreamError::UnexpectedEvent)
    }
}
