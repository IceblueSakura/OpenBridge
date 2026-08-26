//! Bridge lifecycle state machine for Chat Completions SSE chunks.
//!
//! This module accumulates function-call identities and arguments by tool index and requires an
//! explicit `[DONE]` terminal after the finish reason; EOF never counts as successful completion.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::transport::sse::SseEvent;

use super::{
    BridgeStreamError, BridgeToolCall, Lifecycle, StreamTerminal,
    shared::{required_str, required_u64, validate_arguments},
};

/// Per-request state that reconstructs Bridge semantics from the Chat Completions SSE lifecycle.
#[derive(Debug)]
pub struct ChatStreamState {
    lifecycle: Lifecycle,
    finish_reason: Option<String>,
    usage_seen: bool,
    text: String,
    reasoning_text: String,
    tools: BTreeMap<u64, BridgeToolCall>,
}

impl ChatStreamState {
    /// Creates an empty state machine waiting for the first Chat chunk.
    pub fn new() -> Self {
        Self {
            lifecycle: Lifecycle::AwaitingStart,
            finish_reason: None,
            usage_seen: false,
            text: String::new(),
            reasoning_text: String::new(),
            tools: BTreeMap::new(),
        }
    }

    /// Consumes one fully framed Chat SSE event.
    pub fn ingest(&mut self, event: &SseEvent) -> Result<(), BridgeStreamError> {
        self.ingest_event(event).map(|_| ())
    }

    /// Consumes one framed event and classifies chunks needed by protocol conversion.
    pub(crate) fn ingest_event(
        &mut self,
        event: &SseEvent,
    ) -> Result<ChatStreamEventKind, BridgeStreamError> {
        // Handle the Chat `[DONE]` terminal separately and reject early termination without a finish reason.
        if event.data() == "[DONE]" {
            if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
                return Err(BridgeStreamError::DuplicateTerminal);
            }
            if self.finish_reason.is_none() {
                return Err(BridgeStreamError::IncompleteOutputItem);
            }
            self.validate_all_arguments()?;
            self.lifecycle = Lifecycle::Terminal(StreamTerminal::Completed);
            return Ok(ChatStreamEventKind::Terminal);
        }
        if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
            return Err(BridgeStreamError::UnexpectedEvent);
        }

        // Parse the event before distinguishing a normal choice from the optional trailing usage chunk.
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeStreamError::InvalidJson)?;
        let choices = value
            .get("choices")
            .and_then(Value::as_array)
            .ok_or(BridgeStreamError::InvalidJson)?;
        if let Some(finish_reason) = self.finish_reason.as_deref() {
            if value.get("usage").is_some_and(Value::is_object)
                && !self.usage_seen
                && is_post_finish_usage_choices(choices, finish_reason)?
            {
                self.usage_seen = true;
                return Ok(ChatStreamEventKind::Usage);
            }
            return Err(BridgeStreamError::UnexpectedEvent);
        }

        // Update text and tool accumulators from the single normal choice in delta order.
        if choices.len() != 1 || required_u64(&choices[0], "index")? != 0 {
            return Err(BridgeStreamError::UnexpectedEvent);
        }
        self.lifecycle = Lifecycle::Streaming;
        let choice = &choices[0];
        let delta = choice.get("delta").ok_or(BridgeStreamError::InvalidJson)?;
        if let Some(value) = delta.get("content").filter(|value| !value.is_null()) {
            let content = value.as_str().ok_or(BridgeStreamError::InvalidJson)?;
            self.text.push_str(content);
        }
        if let Some(value) = delta
            .get("reasoning_content")
            .filter(|value| !value.is_null())
        {
            let reasoning = value.as_str().ok_or(BridgeStreamError::InvalidJson)?;
            self.reasoning_text.push_str(reasoning);
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                self.ingest_tool_delta(tool_call)?;
            }
        }

        // The finish reason fixes output semantics, but `[DONE]` is the Chat stream terminal.
        if let Some(value) = choice.get("finish_reason").filter(|value| !value.is_null()) {
            let finish_reason = value.as_str().ok_or(BridgeStreamError::InvalidJson)?;
            if !matches!(finish_reason, "stop" | "tool_calls") {
                return Err(BridgeStreamError::UnexpectedEvent);
            }
            if self.finish_reason.is_some() {
                return Err(BridgeStreamError::DuplicateTerminal);
            }
            if (finish_reason == "tool_calls") != !self.tools.is_empty() {
                return Err(BridgeStreamError::UnexpectedEvent);
            }
            self.finish_reason = Some(finish_reason.to_owned());
            self.validate_all_arguments()?;
        }
        Ok(ChatStreamEventKind::Chunk)
    }

    /// Verifies that `[DONE]` appeared when the upstream reaches EOF.
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

    /// Returns assistant text concatenated in chunk order.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns provider reasoning text concatenated in chunk order.
    pub fn reasoning_text(&self) -> &str {
        &self.reasoning_text
    }

    /// Returns function tool calls ordered by Chat tool index.
    pub fn tool_calls(&self) -> Vec<&BridgeToolCall> {
        self.tools.values().collect()
    }

    /// Accumulates a tool-call delta and fixes its identity at first appearance.
    fn ingest_tool_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // Use the index to associate fragments within the stream, but fix the call ID and name on first appearance.
        let index = required_u64(value, "index")?;
        let function = value
            .get("function")
            .ok_or(BridgeStreamError::InvalidJson)?;
        if let Some(existing) = self.tools.get_mut(&index) {
            if value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id != existing.call_id)
                || function
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name != existing.name)
            {
                return Err(BridgeStreamError::IdentityConflict);
            }
            if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                existing.arguments.push_str(arguments);
            }
            return Ok(());
        }

        // A new tool index must carry both a stable call ID and function name.
        let call_id = required_str(value, "id")?.to_owned();
        if self.tools.values().any(|tool| tool.call_id == call_id) {
            return Err(BridgeStreamError::DuplicateIdentity);
        }
        self.tools.insert(
            index,
            BridgeToolCall {
                output_index: index,
                item_id: None,
                call_id,
                name: required_str(function, "name")?.to_owned(),
                arguments: function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                completed: false,
            },
        );
        Ok(())
    }

    /// Validates and completes all tool arguments at the finish-reason or terminal boundary.
    fn validate_all_arguments(&mut self) -> Result<(), BridgeStreamError> {
        // Validate every argument and mark it complete at the finish-reason or terminal boundary.
        for tool in self.tools.values_mut() {
            validate_arguments(&tool.arguments)?;
            tool.completed = true;
        }
        Ok(())
    }
}

/// Accepts either the standard empty-choice usage tail or OpenRouter's inert repeated finish choice.
fn is_post_finish_usage_choices(
    choices: &[Value],
    finish_reason: &str,
) -> Result<bool, BridgeStreamError> {
    if choices.is_empty() {
        return Ok(true);
    }
    if choices.len() != 1 || required_u64(&choices[0], "index")? != 0 {
        return Ok(false);
    }
    let choice = choices[0]
        .as_object()
        .ok_or(BridgeStreamError::InvalidJson)?;
    if choice.get("finish_reason").and_then(Value::as_str) != Some(finish_reason) {
        return Ok(false);
    }
    let delta = choice
        .get("delta")
        .and_then(Value::as_object)
        .ok_or(BridgeStreamError::InvalidJson)?;
    Ok(delta.iter().all(|(field, value)| match field.as_str() {
        "content" | "reasoning" | "reasoning_content" | "refusal" => {
            value.is_null() || value.as_str() == Some("")
        }
        "role" => value.is_null() || value.as_str() == Some("assistant"),
        "tool_calls" => value.is_null() || value.as_array().is_some_and(Vec::is_empty),
        _ => false,
    }))
}

/// Chat stream event classes needed by bridge renderers after lifecycle validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChatStreamEventKind {
    /// A normal single-choice Chat delta.
    Chunk,
    /// The optional usage-only chunk after a successful finish reason.
    Usage,
    /// The explicit `[DONE]` terminal.
    Terminal,
}

impl Default for ChatStreamState {
    fn default() -> Self {
        Self::new()
    }
}
