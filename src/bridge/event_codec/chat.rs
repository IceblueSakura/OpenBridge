//! Chat Completions wire-event decoder and target encoder.

use std::collections::BTreeMap;

use bytes::Bytes;
use serde_json::{Map, Value, json};

use crate::{
    core::ReasoningOutput,
    ir::generation::{
        CandidateIdentity, CandidateRef, EventEnvelope, EventLimits, FinishReason, GenerationEvent,
        ItemHeader, ItemId, ItemIdentity, ItemRef, MessageRole, OutputIndex, PartDelta, PartId,
        PartIdentity, PartKind, PartRef, ResponseIdentity, TerminalStatus, TurnTerminal, Usage,
    },
    transport::sse::SseEvent,
};

use super::{
    StaticEventCodecError,
    shared::{
        bridge_item_id, call_id, candidate_id, envelope, item_id, map_id, parse_object, part_id,
        required_string, required_u64, response_id, sse_data, text, tool_name, usage_from_chat,
    },
};

#[derive(Clone, Debug)]
struct ChatTool {
    item: ItemId,
    part: PartId,
    call: crate::ir::generation::CallId,
    name: crate::ir::generation::ToolName,
    arguments: String,
}

/// Stateful Chat decoder that emits only canonical Event IR values.
pub(super) struct ChatEventDecoder {
    limits: EventLimits,
    reasoning_output: ReasoningOutput,
    sequence: u64,
    upstream_id: Option<String>,
    candidate: Option<crate::ir::generation::CandidateId>,
    message: Option<(ItemId, PartId)>,
    reasoning: Option<(ItemId, PartId)>,
    reasoning_seen: bool,
    tools: BTreeMap<u64, ChatTool>,
    finish: Option<FinishReason>,
    terminal: bool,
    usage_seen: bool,
}

impl ChatEventDecoder {
    pub(super) fn new(limits: EventLimits, reasoning_output: ReasoningOutput) -> Self {
        Self {
            limits,
            reasoning_output,
            sequence: 0,
            upstream_id: None,
            candidate: None,
            message: None,
            reasoning: None,
            reasoning_seen: false,
            tools: BTreeMap::new(),
            finish: None,
            terminal: false,
            usage_seen: false,
        }
    }

    pub(super) fn decode(
        &mut self,
        event: &SseEvent,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if self.terminal {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        if event.event().is_some() {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        if event.data() == "[DONE]" {
            if self.finish.is_none() {
                return Err(StaticEventCodecError::InvalidLifecycle);
            }
            self.terminal = true;
            return Ok(vec![envelope(
                &mut self.sequence,
                GenerationEvent::Terminal {
                    terminal: TurnTerminal::new(TerminalStatus::Completed, None),
                },
            )?]);
        }
        let value = parse_object(event.data(), self.limits)?;
        let choices = value
            .get("choices")
            .and_then(Value::as_array)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if self.finish.is_some() {
            return self.decode_usage_tail(&value, choices);
        }
        if choices.len() != 1 {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        let choice = choices[0]
            .as_object()
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if required_u64(choice, "index")? != 0 {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        let upstream_id = required_string(&value, "id")?;
        let mut events = self.ensure_started(&upstream_id)?;
        let delta = choice
            .get("delta")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if let Some(role) = delta.get("role").filter(|value| !value.is_null())
            && role.as_str() != Some("assistant")
        {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }

        if let Some(reasoning) = delta
            .get("reasoning_content")
            .filter(|value| !value.is_null())
        {
            let reasoning = reasoning
                .as_str()
                .ok_or(StaticEventCodecError::InvalidJson)?;
            if !reasoning.is_empty() {
                if !self.reasoning_output.is_readable()
                    || self.message.is_some()
                    || !self.tools.is_empty()
                {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
                events.extend(self.reasoning_delta(reasoning)?);
            }
        }
        if let Some(content) = delta.get("content").filter(|value| !value.is_null()) {
            let content = content.as_str().ok_or(StaticEventCodecError::InvalidJson)?;
            if !content.is_empty() {
                if !self.tools.is_empty() {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
                events.extend(self.close_reasoning()?);
                events.extend(self.message_delta(content)?);
            }
        }
        if let Some(tool_calls) = delta.get("tool_calls").filter(|value| !value.is_null()) {
            let tool_calls = tool_calls
                .as_array()
                .ok_or(StaticEventCodecError::InvalidJson)?;
            if self.message.is_some() {
                return Err(StaticEventCodecError::UnsupportedSemantics);
            }
            events.extend(self.close_reasoning()?);
            for tool_call in tool_calls {
                events.extend(self.tool_delta(tool_call)?);
            }
        }

        if let Some(reason) = choice.get("finish_reason").filter(|value| !value.is_null()) {
            let reason = reason.as_str().ok_or(StaticEventCodecError::InvalidJson)?;
            let finish = match reason {
                "stop" => FinishReason::Stop,
                "tool_calls" => FinishReason::ToolCalls,
                _ => return Err(StaticEventCodecError::UnsupportedSemantics),
            };
            if (finish == FinishReason::ToolCalls) != !self.tools.is_empty() {
                return Err(StaticEventCodecError::InvalidLifecycle);
            }
            events.extend(self.close_reasoning()?);
            events.extend(self.close_outputs()?);
            let candidate = self.candidate()?.clone();
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::CandidateFinished {
                    candidate: CandidateRef::new(candidate),
                    finish: finish.clone(),
                },
            )?);
            self.finish = Some(finish);
        }
        if let Some(usage) = value.get("usage").and_then(Value::as_object) {
            events.push(self.usage_event(usage)?);
        }
        Ok(events)
    }

    pub(super) fn finish(&self) -> Result<(), StaticEventCodecError> {
        if self.terminal {
            Ok(())
        } else {
            Err(StaticEventCodecError::EofBeforeTerminal)
        }
    }

    fn ensure_started(
        &mut self,
        upstream_id: &str,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if let Some(known) = &self.upstream_id {
            if known != upstream_id {
                return Err(StaticEventCodecError::IdentityConflict);
            }
            return Ok(Vec::new());
        }
        let response = response_id(map_id(upstream_id, "chatcmpl_", "resp_"), self.limits)?;
        let candidate = candidate_id("candidate_0", self.limits)?;
        self.upstream_id = Some(upstream_id.to_owned());
        self.candidate = Some(candidate.clone());
        Ok(vec![
            envelope(
                &mut self.sequence,
                GenerationEvent::ResponseStarted {
                    response: ResponseIdentity::new(response),
                },
            )?,
            envelope(
                &mut self.sequence,
                GenerationEvent::CandidateStarted {
                    candidate: CandidateIdentity::new(candidate, OutputIndex::new(0)),
                },
            )?,
        ])
    }

    fn reasoning_delta(
        &mut self,
        delta: &str,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let mut events = Vec::new();
        if self.reasoning.is_none() {
            let suffix = self
                .upstream_id
                .as_deref()
                .and_then(|id| id.strip_prefix("chatcmpl_"))
                .unwrap_or(self.upstream_id.as_deref().unwrap_or("response"));
            let item = item_id(format!("rs_{suffix}"), self.limits)?;
            let part = part_id(format!("{}:reasoning", item.as_str()), self.limits)?;
            events.extend(self.start_item(
                item.clone(),
                part.clone(),
                OutputIndex::new(0),
                ItemHeader::Reasoning,
                PartKind::ReasoningText,
            )?);
            self.reasoning = Some((item, part));
            self.reasoning_seen = true;
        }
        let part = self.reasoning.as_ref().unwrap().1.clone();
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::PartDelta {
                part: PartRef::new(part),
                delta: PartDelta::ReasoningText(text(delta, self.limits)?),
            },
        )?);
        Ok(events)
    }

    fn message_delta(&mut self, delta: &str) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let mut events = Vec::new();
        if self.message.is_none() {
            let suffix = self
                .upstream_id
                .as_deref()
                .and_then(|id| id.strip_prefix("chatcmpl_"))
                .unwrap_or(self.upstream_id.as_deref().unwrap_or("response"));
            let item = item_id(format!("msg_{suffix}"), self.limits)?;
            let part = part_id(format!("{}:text", item.as_str()), self.limits)?;
            let index = u64::from(self.reasoning_seen);
            events.extend(self.start_item(
                item.clone(),
                part.clone(),
                OutputIndex::new(index),
                ItemHeader::Message {
                    role: MessageRole::Assistant,
                },
                PartKind::Text,
            )?);
            self.message = Some((item, part));
        }
        let part = self.message.as_ref().unwrap().1.clone();
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::PartDelta {
                part: PartRef::new(part),
                delta: PartDelta::Text(text(delta, self.limits)?),
            },
        )?);
        Ok(events)
    }

    fn tool_delta(&mut self, value: &Value) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let value = value
            .as_object()
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let index = required_u64(value, "index")?;
        let function = value
            .get("function")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let reasoning_offset = u64::from(self.reasoning_seen);
        let mut events = Vec::new();
        if !self.tools.contains_key(&index) {
            let call = call_id(required_string(value, "id")?, self.limits)?;
            if self.tools.values().any(|known| known.call == call) {
                return Err(StaticEventCodecError::DuplicateIdentity);
            }
            let name = tool_name(required_string(function, "name")?, self.limits)?;
            let item = item_id(bridge_item_id(call.as_str()), self.limits)?;
            let part = part_id(format!("{}:arguments", item.as_str()), self.limits)?;
            let candidate = self.candidate()?.clone();
            // Provider indexes are untrusted: reject overflow instead of wrapping candidate positions.
            let output_index = index
                .checked_add(reasoning_offset)
                .ok_or(StaticEventCodecError::LimitExceeded)?;
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::ItemStarted {
                    candidate: CandidateRef::new(candidate),
                    item: ItemIdentity::new(item.clone(), OutputIndex::new(output_index), None),
                    header: ItemHeader::ToolCall {
                        call: call.clone(),
                        tool: name.clone(),
                    },
                },
            )?);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartStarted {
                    item: ItemRef::new(item.clone()),
                    part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                    kind: PartKind::ToolArguments,
                },
            )?);
            self.tools.insert(
                index,
                ChatTool {
                    item,
                    part,
                    call,
                    name,
                    arguments: String::new(),
                },
            );
        } else {
            let known = self.tools.get(&index).unwrap();
            if value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id != known.call.as_str())
                || function
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name != known.name.as_str())
            {
                return Err(StaticEventCodecError::IdentityConflict);
            }
        }
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !arguments.is_empty() {
            let tool = self.tools.get_mut(&index).unwrap();
            tool.arguments.push_str(arguments);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartDelta {
                    part: PartRef::new(tool.part.clone()),
                    delta: PartDelta::ToolArguments(text(arguments, self.limits)?),
                },
            )?);
        }
        Ok(events)
    }

    fn start_item(
        &mut self,
        item: ItemId,
        part: PartId,
        index: OutputIndex,
        header: ItemHeader,
        kind: PartKind,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let candidate = self.candidate()?.clone();
        Ok(vec![
            envelope(
                &mut self.sequence,
                GenerationEvent::ItemStarted {
                    candidate: CandidateRef::new(candidate),
                    item: ItemIdentity::new(item.clone(), index, None),
                    header,
                },
            )?,
            envelope(
                &mut self.sequence,
                GenerationEvent::PartStarted {
                    item: ItemRef::new(item),
                    part: PartIdentity::new(part, OutputIndex::new(0)),
                    kind,
                },
            )?,
        ])
    }

    fn close_reasoning(&mut self) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let Some((item, part)) = self.reasoning.take() else {
            return Ok(Vec::new());
        };
        self.close_item(item, part)
    }

    fn close_outputs(&mut self) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let mut events = Vec::new();
        if let Some((item, part)) = self.message.take() {
            events.extend(self.close_item(item, part)?);
        }
        let tools = self.tools.values().cloned().collect::<Vec<_>>();
        for tool in tools {
            events.extend(self.close_item(tool.item, tool.part)?);
        }
        Ok(events)
    }

    fn close_item(
        &mut self,
        item: ItemId,
        part: PartId,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        Ok(vec![
            envelope(
                &mut self.sequence,
                GenerationEvent::PartFinished {
                    part: PartRef::new(part),
                },
            )?,
            envelope(
                &mut self.sequence,
                GenerationEvent::ItemFinished {
                    item: ItemRef::new(item),
                },
            )?,
        ])
    }

    fn decode_usage_tail(
        &mut self,
        value: &Map<String, Value>,
        choices: &[Value],
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if self.usage_seen
            || !choices.is_empty()
                && !is_inert_finish_choice(choices, self.finish.as_ref().unwrap())?
        {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        if value.get("id").and_then(Value::as_str) != self.upstream_id.as_deref() {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let usage = value
            .get("usage")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        self.usage_seen = true;
        Ok(vec![self.usage_event(usage)?])
    }

    fn usage_event(
        &mut self,
        usage: &Map<String, Value>,
    ) -> Result<EventEnvelope, StaticEventCodecError> {
        envelope(
            &mut self.sequence,
            GenerationEvent::UsageSnapshot {
                usage: usage_from_chat(usage)?,
            },
        )
    }

    fn candidate(&self) -> Result<&crate::ir::generation::CandidateId, StaticEventCodecError> {
        self.candidate
            .as_ref()
            .ok_or(StaticEventCodecError::InvalidLifecycle)
    }
}

fn is_inert_finish_choice(
    choices: &[Value],
    finish: &FinishReason,
) -> Result<bool, StaticEventCodecError> {
    if choices.len() != 1 {
        return Ok(false);
    }
    let choice = choices[0]
        .as_object()
        .ok_or(StaticEventCodecError::InvalidJson)?;
    if required_u64(choice, "index")? != 0
        || choice.get("finish_reason").and_then(Value::as_str)
            != Some(match finish {
                FinishReason::Stop => "stop",
                FinishReason::ToolCalls => "tool_calls",
                _ => return Ok(false),
            })
    {
        return Ok(false);
    }
    let delta = choice
        .get("delta")
        .and_then(Value::as_object)
        .ok_or(StaticEventCodecError::InvalidJson)?;
    Ok(delta.values().all(|value| {
        value.is_null()
            || value.as_str() == Some("")
            || value.as_array().is_some_and(Vec::is_empty)
            || value.as_str() == Some("assistant")
    }))
}

#[derive(Clone, Debug)]
enum ChatPartTarget {
    Text,
    Reasoning,
    Tool { chat_index: u64 },
    Opaque,
}

#[derive(Clone, Debug)]
struct ChatItemTarget {
    chat_index: Option<u64>,
}

/// Canonical Event IR to Chat SSE encoder.
pub(super) struct ChatEventEncoder {
    limits: EventLimits,
    model: String,
    include_usage: bool,
    reasoning_supported: bool,
    chat_id: Option<String>,
    role_emitted: bool,
    items: BTreeMap<ItemId, ChatItemTarget>,
    parts: BTreeMap<PartId, ChatPartTarget>,
    next_tool_index: u64,
    has_text: bool,
    has_reasoning: bool,
    has_tools: bool,
    finish: Option<FinishReason>,
    usage: Option<Usage>,
    terminal: bool,
}

impl ChatEventEncoder {
    pub(super) fn new(
        limits: EventLimits,
        model: &str,
        include_usage: bool,
        reasoning_output: ReasoningOutput,
    ) -> Self {
        Self {
            limits,
            model: model.to_owned(),
            include_usage,
            reasoning_supported: reasoning_output.is_readable(),
            chat_id: None,
            role_emitted: false,
            items: BTreeMap::new(),
            parts: BTreeMap::new(),
            next_tool_index: 0,
            has_text: false,
            has_reasoning: false,
            has_tools: false,
            finish: None,
            usage: None,
            terminal: false,
        }
    }

    pub(super) fn encode(
        &mut self,
        event: &GenerationEvent,
    ) -> Result<Bytes, StaticEventCodecError> {
        match event {
            GenerationEvent::ResponseStarted { response } => {
                if self.chat_id.is_some() {
                    return Err(StaticEventCodecError::InvalidLifecycle);
                }
                self.chat_id = Some(map_id(response.id().as_str(), "resp_", "chatcmpl_"));
                Ok(Bytes::new())
            }
            GenerationEvent::CandidateStarted { candidate } => {
                if candidate.index().get() != 0 {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
                Ok(Bytes::new())
            }
            GenerationEvent::ItemStarted { item, header, .. } => self.start_item(item, header),
            GenerationEvent::PartStarted { item, part, kind } => self.start_part(item, part, *kind),
            GenerationEvent::PartDelta { part, delta } => self.delta(part, delta),
            GenerationEvent::PartFinished { .. } | GenerationEvent::ItemFinished { .. } => {
                Ok(Bytes::new())
            }
            GenerationEvent::CandidateFinished { finish, .. } => {
                self.finish = Some(finish.clone());
                Ok(Bytes::new())
            }
            GenerationEvent::UsageSnapshot { usage } => {
                self.usage = Some(*usage);
                Ok(Bytes::new())
            }
            GenerationEvent::Terminal { terminal } => self.terminal(terminal),
        }
    }

    pub(super) fn finish(&self) -> Result<(), StaticEventCodecError> {
        if self.terminal {
            Ok(())
        } else {
            Err(StaticEventCodecError::EofBeforeTerminal)
        }
    }

    fn start_item(
        &mut self,
        item: &ItemIdentity,
        header: &ItemHeader,
    ) -> Result<Bytes, StaticEventCodecError> {
        let target = match header {
            ItemHeader::Message {
                role: MessageRole::Assistant,
            } => ChatItemTarget { chat_index: None },
            ItemHeader::Message { .. } => {
                return Err(StaticEventCodecError::UnsupportedSemantics);
            }
            ItemHeader::Reasoning => {
                if !self.reasoning_supported {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
                ChatItemTarget { chat_index: None }
            }
            ItemHeader::ToolCall { call, tool } => {
                let chat_index = self.next_tool_index;
                self.next_tool_index = self
                    .next_tool_index
                    .checked_add(1)
                    .ok_or(StaticEventCodecError::InvalidLifecycle)?;
                self.has_tools = true;
                let mut delta = Map::new();
                if !self.role_emitted {
                    delta.insert("role".to_owned(), Value::String("assistant".to_owned()));
                    self.role_emitted = true;
                }
                delta.insert(
                    "tool_calls".to_owned(),
                    json!([{
                        "function": {"arguments": "", "name": tool.as_str()},
                        "id": call.as_str(),
                        "index": chat_index,
                        "type": "function"
                    }]),
                );
                let bytes = self.chat_chunk(Value::Object(delta), Value::Null)?;
                self.items.insert(
                    item.id().clone(),
                    ChatItemTarget {
                        chat_index: Some(chat_index),
                    },
                );
                return Ok(bytes);
            }
        };
        if self.items.insert(item.id().clone(), target).is_some() {
            return Err(StaticEventCodecError::DuplicateIdentity);
        }
        Ok(Bytes::new())
    }

    fn start_part(
        &mut self,
        item: &ItemRef,
        part: &PartIdentity,
        kind: PartKind,
    ) -> Result<Bytes, StaticEventCodecError> {
        let item_target = self
            .items
            .get(item.id())
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        let target = match kind {
            PartKind::Text => ChatPartTarget::Text,
            PartKind::ReasoningText | PartKind::ReasoningSummary => ChatPartTarget::Reasoning,
            PartKind::ToolArguments => ChatPartTarget::Tool {
                chat_index: item_target
                    .chat_index
                    .ok_or(StaticEventCodecError::IdentityConflict)?,
            },
            PartKind::Opaque => ChatPartTarget::Opaque,
        };
        if self.parts.insert(part.id().clone(), target).is_some() {
            return Err(StaticEventCodecError::DuplicateIdentity);
        }
        Ok(Bytes::new())
    }

    fn delta(&mut self, part: &PartRef, delta: &PartDelta) -> Result<Bytes, StaticEventCodecError> {
        match self
            .parts
            .get(part.id())
            .ok_or(StaticEventCodecError::IdentityConflict)?
        {
            ChatPartTarget::Text => {
                let PartDelta::Text(value) = delta else {
                    return Err(StaticEventCodecError::IdentityConflict);
                };
                self.has_text = true;
                self.text_chunk("content", value.as_str())
            }
            ChatPartTarget::Reasoning => {
                let value = match delta {
                    PartDelta::ReasoningText(value) | PartDelta::ReasoningSummary(value) => value,
                    _ => return Err(StaticEventCodecError::IdentityConflict),
                };
                self.has_reasoning = true;
                self.text_chunk("reasoning_content", value.as_str())
            }
            ChatPartTarget::Tool { chat_index } => {
                let PartDelta::ToolArguments(value) = delta else {
                    return Err(StaticEventCodecError::IdentityConflict);
                };
                self.chat_chunk(
                    json!({"tool_calls": [{
                        "function": {"arguments": value.as_str()},
                        "index": chat_index
                    }]}),
                    Value::Null,
                )
            }
            ChatPartTarget::Opaque => {
                if !matches!(delta, PartDelta::Opaque(_)) {
                    return Err(StaticEventCodecError::IdentityConflict);
                }
                Ok(Bytes::new())
            }
        }
    }

    fn text_chunk(&mut self, field: &str, value: &str) -> Result<Bytes, StaticEventCodecError> {
        let mut delta = Map::new();
        if !self.role_emitted {
            delta.insert("role".to_owned(), Value::String("assistant".to_owned()));
            self.role_emitted = true;
        }
        delta.insert(field.to_owned(), Value::String(value.to_owned()));
        self.chat_chunk(Value::Object(delta), Value::Null)
    }

    fn terminal(&mut self, terminal: &TurnTerminal) -> Result<Bytes, StaticEventCodecError> {
        if terminal.status() != TerminalStatus::Completed
            || self.terminal
            || (!self.has_text && !self.has_reasoning && !self.has_tools)
        {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let finish = match self.finish.as_ref() {
            Some(FinishReason::ToolCalls) => "tool_calls",
            Some(FinishReason::Stop) => "stop",
            _ => return Err(StaticEventCodecError::UnsupportedSemantics),
        };
        let mut output = self
            .chat_chunk(json!({}), Value::String(finish.to_owned()))?
            .to_vec();
        if self.include_usage {
            let usage = self
                .usage
                .as_ref()
                .ok_or(StaticEventCodecError::InvalidLifecycle)?;
            output.extend(self.usage_chunk(usage)?.to_vec());
        }
        output.extend_from_slice(b"data: [DONE]\n\n");
        self.terminal = true;
        Ok(Bytes::from(output))
    }

    fn chat_chunk(
        &self,
        delta: Value,
        finish_reason: Value,
    ) -> Result<Bytes, StaticEventCodecError> {
        let mut chunk = json!({
            "choices": [{"delta": delta, "finish_reason": finish_reason, "index": 0}],
            "id": self.chat_id()?,
            "model": self.model,
            "object": "chat.completion.chunk"
        });
        if self.include_usage {
            chunk["usage"] = Value::Null;
        }
        sse_data(&chunk, self.limits)
    }

    fn usage_chunk(&self, usage: &Usage) -> Result<Bytes, StaticEventCodecError> {
        let mut encoded = json!({
            "completion_tokens": usage.output_tokens(),
            "prompt_tokens": usage.input_tokens(),
            "total_tokens": usage.total_tokens()
        });
        if let Some(value) = usage.reasoning_tokens() {
            encoded["completion_tokens_details"] = json!({"reasoning_tokens": value});
        }
        if let Some(value) = usage.cached_input_tokens() {
            encoded["prompt_tokens_details"] = json!({"cached_tokens": value});
        }
        sse_data(
            &json!({
                "choices": [],
                "id": self.chat_id()?,
                "model": self.model,
                "object": "chat.completion.chunk",
                "usage": encoded
            }),
            self.limits,
        )
    }

    fn chat_id(&self) -> Result<&str, StaticEventCodecError> {
        self.chat_id
            .as_deref()
            .ok_or(StaticEventCodecError::InvalidLifecycle)
    }
}
