//! Responses wire-event decoder and target encoder.

use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;
use serde_json::{Map, Value, json};

use crate::{
    ir::generation::{
        BoundedBytes, BoundedOpaqueJson, CandidateIdentity, CandidateRef, EventEnvelope,
        EventLimits, ExtensionKind, FinishReason, GenerationEvent, ItemHeader, ItemId,
        ItemIdentity, ItemRef, MessageRole, OpaquePayload, OutputIndex, PartDelta, PartId,
        PartIdentity, PartKind, PartRef, ProviderExtension, ProviderNamespace, ResponseIdentity,
        TerminalStatus, TurnTerminal, Usage,
    },
    transport::sse::SseEvent,
};

use super::{
    StaticEventCodecError,
    shared::{
        call_id, candidate_id, envelope, item_id, parse_object, part_id, required_string,
        required_u64, response_event, response_id, text, tool_name, usage_from_responses,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResponsesItemKind {
    Message,
    Reasoning,
    Tool,
}

#[derive(Clone, Debug)]
struct ResponsesWireItem {
    item: ItemId,
    part: Option<PartId>,
    kind: ResponsesItemKind,
    part_kind: Option<PartKind>,
    value: String,
    opaque: Option<String>,
    call_id: Option<crate::ir::generation::CallId>,
    tool_name: Option<crate::ir::generation::ToolName>,
    finished: bool,
}

/// Stateful Responses decoder that emits only canonical Event IR values.
pub(super) struct ResponsesEventDecoder {
    limits: EventLimits,
    native: bool,
    sequence: u64,
    response_id: Option<crate::ir::generation::ResponseId>,
    candidate: Option<crate::ir::generation::CandidateId>,
    items: BTreeMap<u64, ResponsesWireItem>,
    item_ids: BTreeSet<ItemId>,
    call_ids: BTreeSet<crate::ir::generation::CallId>,
    terminal: bool,
}

impl ResponsesEventDecoder {
    pub(super) fn new(limits: EventLimits, native: bool) -> Self {
        Self {
            limits,
            native,
            sequence: 0,
            response_id: None,
            candidate: None,
            items: BTreeMap::new(),
            item_ids: BTreeSet::new(),
            call_ids: BTreeSet::new(),
            terminal: false,
        }
    }

    pub(super) fn decode(
        &mut self,
        event: &SseEvent,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if self.terminal {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let mut value = parse_object(event.data(), self.limits)?;
        if value
            .get("type")
            .is_some_and(|value| value.as_str().is_none_or(str::is_empty))
        {
            return Err(StaticEventCodecError::InvalidJson);
        }
        let kind = match (
            event.event().filter(|name| !name.is_empty()),
            value.get("type").and_then(Value::as_str),
        ) {
            (Some(event_kind), Some(data_kind)) if event_kind != data_kind => {
                return Err(StaticEventCodecError::IdentityConflict);
            }
            (Some(event_kind), _) => event_kind.to_owned(),
            (None, Some(data_kind)) => data_kind.to_owned(),
            (None, None) => return Err(StaticEventCodecError::InvalidJson),
        };
        if value.get("type").is_none() {
            value.insert("type".to_owned(), Value::String(kind.clone()));
        }
        match kind.as_str() {
            "response.created" => self.created(&value),
            "response.in_progress" => self.in_progress(&value),
            "response.output_item.added" => self.item_added(&value),
            "response.content_part.added" => self.content_part(&value, false),
            "response.content_part.done" => self.content_part(&value, true),
            "response.output_text.delta" => self.delta(&value, PartKind::Text, "delta"),
            "response.output_text.done" => self
                .validate_done(&value, PartKind::Text, "text")
                .map(|()| Vec::new()),
            "response.refusal.delta" => self.delta(&value, PartKind::Refusal, "delta"),
            "response.refusal.done" => self
                .validate_done(&value, PartKind::Refusal, "refusal")
                .map(|()| Vec::new()),
            "response.reasoning_summary_part.added" => self.reasoning_part(&value, false),
            "response.reasoning_summary_part.done" => self.reasoning_part(&value, true),
            "response.reasoning_summary_text.delta" => {
                self.delta(&value, PartKind::ReasoningSummary, "delta")
            }
            "response.reasoning_text.delta" => self.delta(&value, PartKind::ReasoningText, "delta"),
            "response.reasoning_summary_text.done" => self
                .validate_done(&value, PartKind::ReasoningSummary, "text")
                .map(|()| Vec::new()),
            "response.reasoning_text.done" => self
                .validate_done(&value, PartKind::ReasoningText, "text")
                .map(|()| Vec::new()),
            "response.function_call_arguments.delta" => {
                self.delta(&value, PartKind::ToolArguments, "delta")
            }
            "response.function_call_arguments.done" => self
                .validate_done(&value, PartKind::ToolArguments, "arguments")
                .map(|()| Vec::new()),
            "response.output_item.done" => self.item_done(&value),
            "response.completed" => self.completed(&value),
            "response.failed" => self.non_completed(&value, TerminalStatus::Failed),
            "response.incomplete" => self.non_completed(&value, TerminalStatus::Incomplete),
            "response.cancelled" => self.non_completed(&value, TerminalStatus::Cancelled),
            "error" => self.error_terminal(),
            _ => self.extension(&kind, value),
        }
    }

    pub(super) fn finish(&self) -> Result<(), StaticEventCodecError> {
        if self.terminal {
            Ok(())
        } else {
            Err(StaticEventCodecError::EofBeforeTerminal)
        }
    }

    fn created(
        &mut self,
        value: &Map<String, Value>,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if self.response_id.is_some() {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let response = value
            .get("response")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if response.get("status").and_then(Value::as_str) != Some("in_progress") {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        if response
            .get("output")
            .is_some_and(|output| !output.as_array().is_some_and(Vec::is_empty))
        {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        let response = response_id(required_string(response, "id")?, self.limits)?;
        let candidate = candidate_id("candidate_0", self.limits)?;
        self.response_id = Some(response.clone());
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

    fn in_progress(
        &self,
        value: &Map<String, Value>,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let response = value
            .get("response")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if response.get("id").and_then(Value::as_str)
            != self.response_id.as_ref().map(|id| id.as_str())
            || response.get("status").and_then(Value::as_str) != Some("in_progress")
        {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        Ok(Vec::new())
    }

    fn item_added(
        &mut self,
        value: &Map<String, Value>,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        self.ensure_started()?;
        let index = required_u64(value, "output_index")?;
        if self.items.contains_key(&index) {
            return Err(StaticEventCodecError::DuplicateIdentity);
        }
        let item = value
            .get("item")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if item
            .get("status")
            .is_some_and(|status| status.as_str() != Some("in_progress"))
        {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let item_value = item_id(required_string(item, "id")?, self.limits)?;
        if !self.item_ids.insert(item_value.clone()) {
            return Err(StaticEventCodecError::DuplicateIdentity);
        }
        let output_index = OutputIndex::new(index);
        let mut events = Vec::new();
        let item_type = required_string(item, "type")?;
        match item_type.as_str() {
            "message" => {
                if item.get("role").and_then(Value::as_str) != Some("assistant")
                    || !empty_array(item, "content")?
                {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
            }
            "reasoning" => {
                if !empty_array(item, "content")?
                    || !empty_array(item, "summary")?
                    || item
                        .get("encrypted_content")
                        .is_some_and(|value| !value.is_null())
                {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
            }
            "function_call" => {}
            _ => return Err(StaticEventCodecError::UnsupportedSemantics),
        }
        let (kind, header, part_kind, call, name) = match item_type.as_str() {
            "message" => (
                ResponsesItemKind::Message,
                ItemHeader::Message {
                    role: MessageRole::Assistant,
                },
                None,
                None,
                None,
            ),
            "reasoning" => (
                ResponsesItemKind::Reasoning,
                ItemHeader::Reasoning,
                None,
                None,
                None,
            ),
            "function_call" => {
                let call = call_id(required_string(item, "call_id")?, self.limits)?;
                if !self.call_ids.insert(call.clone()) {
                    return Err(StaticEventCodecError::DuplicateIdentity);
                }
                let name = tool_name(required_string(item, "name")?, self.limits)?;
                (
                    ResponsesItemKind::Tool,
                    ItemHeader::ToolCall {
                        call: call.clone(),
                        tool: name.clone(),
                    },
                    Some(PartKind::ToolArguments),
                    Some(call),
                    Some(name),
                )
            }
            _ => return Err(StaticEventCodecError::UnsupportedSemantics),
        };
        let candidate = self.candidate()?.clone();
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::ItemStarted {
                candidate: CandidateRef::new(candidate),
                item: ItemIdentity::new(item_value.clone(), output_index, None),
                header,
            },
        )?);
        let mut wire = ResponsesWireItem {
            item: item_value,
            part: None,
            kind,
            part_kind: None,
            value: String::new(),
            opaque: None,
            call_id: call,
            tool_name: name,
            finished: false,
        };
        if let Some(part_kind) = part_kind {
            events.extend(self.start_part(&mut wire, part_kind)?);
        }
        if kind == ResponsesItemKind::Tool
            && let Some(arguments) = item.get("arguments").and_then(Value::as_str)
            && !arguments.is_empty()
        {
            wire.value.push_str(arguments);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartDelta {
                    part: PartRef::new(wire.part.clone().unwrap()),
                    delta: PartDelta::ToolArguments(text(arguments, self.limits)?),
                },
            )?);
        }
        self.items.insert(index, wire);
        Ok(events)
    }

    fn start_part(
        &mut self,
        item: &mut ResponsesWireItem,
        kind: PartKind,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if let Some(existing) = item.part_kind {
            if existing != kind {
                return Err(StaticEventCodecError::IdentityConflict);
            }
            return Ok(Vec::new());
        }
        let suffix = match kind {
            PartKind::Text => "text",
            PartKind::Refusal => "refusal",
            PartKind::ReasoningText => "reasoning",
            PartKind::ReasoningSummary => "summary",
            PartKind::ToolArguments => "arguments",
            PartKind::Opaque => return Err(StaticEventCodecError::UnsupportedSemantics),
        };
        let part = part_id(format!("{}:{suffix}", item.item.as_str()), self.limits)?;
        item.part = Some(part.clone());
        item.part_kind = Some(kind);
        Ok(vec![envelope(
            &mut self.sequence,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item.item.clone()),
                part: PartIdentity::new(part, OutputIndex::new(0)),
                kind,
            },
        )?])
    }

    fn delta(
        &mut self,
        value: &Map<String, Value>,
        kind: PartKind,
        field: &str,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let index = required_u64(value, "output_index")?;
        let item_id = required_string(value, "item_id")?;
        validate_child_index(value, kind)?;
        let delta = required_string(value, field)?;
        let mut item = self
            .items
            .remove(&index)
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        if item.item.as_str() != item_id || item.finished {
            self.items.insert(index, item);
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let mut events = self.start_part(&mut item, kind)?;
        item.value.push_str(&delta);
        let part = item.part.clone().unwrap();
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::PartDelta {
                part: PartRef::new(part),
                delta: match kind {
                    PartKind::Text => PartDelta::Text(text(&delta, self.limits)?),
                    PartKind::Refusal => PartDelta::Refusal(text(&delta, self.limits)?),
                    PartKind::ReasoningText => PartDelta::ReasoningText(text(&delta, self.limits)?),
                    PartKind::ReasoningSummary => {
                        PartDelta::ReasoningSummary(text(&delta, self.limits)?)
                    }
                    PartKind::ToolArguments => PartDelta::ToolArguments(text(&delta, self.limits)?),
                    PartKind::Opaque => return Err(StaticEventCodecError::UnsupportedSemantics),
                },
            },
        )?);
        self.items.insert(index, item);
        Ok(events)
    }

    fn validate_done(
        &self,
        value: &Map<String, Value>,
        kind: PartKind,
        field: &str,
    ) -> Result<(), StaticEventCodecError> {
        validate_child_index(value, kind)?;
        let item = self.bound_item(value)?;
        if item.part_kind != Some(kind)
            || value.get(field).and_then(Value::as_str) != Some(item.value.as_str())
        {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        Ok(())
    }

    fn reasoning_part(
        &mut self,
        value: &Map<String, Value>,
        done: bool,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let index = required_u64(value, "output_index")?;
        let item_id = required_string(value, "item_id")?;
        if required_u64(value, "summary_index")? != 0 {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let part = value
            .get("part")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if part.get("type").and_then(Value::as_str) != Some("summary_text") {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        let snapshot = part
            .get("text")
            .and_then(Value::as_str)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let mut item = self
            .items
            .remove(&index)
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        if item.item.as_str() != item_id
            || item.kind != ResponsesItemKind::Reasoning
            || item.finished
        {
            self.items.insert(index, item);
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let mut events = self.start_part(&mut item, PartKind::ReasoningSummary)?;
        if !done && !snapshot.is_empty() {
            item.value.push_str(snapshot);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartDelta {
                    part: PartRef::new(item.part.clone().unwrap()),
                    delta: PartDelta::ReasoningSummary(text(snapshot, self.limits)?),
                },
            )?);
        } else if done && snapshot != item.value {
            self.items.insert(index, item);
            return Err(StaticEventCodecError::IdentityConflict);
        }
        self.items.insert(index, item);
        Ok(events)
    }

    fn content_part(
        &mut self,
        value: &Map<String, Value>,
        done: bool,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let index = required_u64(value, "output_index")?;
        let item_id = required_string(value, "item_id")?;
        if required_u64(value, "content_index")? != 0 {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let part = value
            .get("part")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let (kind, field) = match part.get("type").and_then(Value::as_str) {
            Some("output_text") => (PartKind::Text, "text"),
            Some("refusal") => (PartKind::Refusal, "refusal"),
            _ => return Err(StaticEventCodecError::UnsupportedSemantics),
        };
        let snapshot = part
            .get(field)
            .and_then(Value::as_str)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let mut item = self
            .items
            .remove(&index)
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        if item.item.as_str() != item_id || item.kind != ResponsesItemKind::Message || item.finished
        {
            self.items.insert(index, item);
            return Err(StaticEventCodecError::IdentityConflict);
        }
        if item.part_kind.is_some_and(|existing| existing != kind) {
            self.items.insert(index, item);
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let mut events = self.start_part(&mut item, kind)?;
        if done {
            if snapshot != item.value {
                self.items.insert(index, item);
                return Err(StaticEventCodecError::IdentityConflict);
            }
            events.extend(self.annotation_events(&json!({"id": item_id, "content": [part]}))?);
        } else if !snapshot.is_empty() {
            item.value.push_str(snapshot);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartDelta {
                    part: PartRef::new(item.part.clone().unwrap()),
                    delta: match kind {
                        PartKind::Text => PartDelta::Text(text(snapshot, self.limits)?),
                        PartKind::Refusal => PartDelta::Refusal(text(snapshot, self.limits)?),
                        _ => return Err(StaticEventCodecError::InvalidLifecycle),
                    },
                },
            )?);
        }
        self.items.insert(index, item);
        Ok(events)
    }

    fn item_done(
        &mut self,
        value: &Map<String, Value>,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let index = required_u64(value, "output_index")?;
        let snapshot = value
            .get("item")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let mut item = self
            .items
            .remove(&index)
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        let status = snapshot.get("status").filter(|value| !value.is_null());
        if item.finished
            || snapshot.get("id").and_then(Value::as_str) != Some(item.item.as_str())
            || status.is_some_and(|value| value.as_str() != Some("completed"))
            || (item.kind == ResponsesItemKind::Message && status.is_none())
        {
            self.items.insert(index, item);
            return Err(StaticEventCodecError::IdentityConflict);
        }
        if item.kind == ResponsesItemKind::Reasoning {
            item.opaque = snapshot
                .get("encrypted_content")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
        validate_item_snapshot(&item, snapshot)?;
        item.finished = true;
        let mut events = Vec::new();
        events.extend(self.annotation_events(&Value::Object(snapshot.clone()))?);
        let next_part_index = if let Some(part) = item.part.clone() {
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartFinished {
                    part: PartRef::new(part),
                },
            )?);
            1
        } else {
            0
        };
        if let Some(encrypted) = item.opaque.as_deref() {
            if item.kind != ResponsesItemKind::Reasoning {
                return Err(StaticEventCodecError::UnsupportedSemantics);
            }
            let opaque = part_id(format!("{}:opaque", item.item.as_str()), self.limits)?;
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartStarted {
                    item: ItemRef::new(item.item.clone()),
                    part: PartIdentity::new(opaque.clone(), OutputIndex::new(next_part_index)),
                    kind: PartKind::Opaque,
                },
            )?);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartDelta {
                    part: PartRef::new(opaque.clone()),
                    delta: PartDelta::Opaque(
                        BoundedBytes::from_slice(
                            encrypted.as_bytes(),
                            self.limits.max_event_bytes(),
                        )
                        .map_err(|_| StaticEventCodecError::LimitExceeded)?,
                    ),
                },
            )?);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::PartFinished {
                    part: PartRef::new(opaque),
                },
            )?);
        }
        if item.part.is_none() && events.is_empty() && item.kind != ResponsesItemKind::Message {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::ItemFinished {
                item: ItemRef::new(item.item.clone()),
            },
        )?);
        self.items.insert(index, item);
        Ok(events)
    }

    fn completed(
        &mut self,
        value: &Map<String, Value>,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let response = value
            .get("response")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        if response.get("status").and_then(Value::as_str) != Some("completed") {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        if ["error", "incomplete_details"]
            .iter()
            .any(|field| response.get(*field).is_some_and(|value| !value.is_null()))
        {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let mut events = Vec::new();
        if self.response_id.is_none() {
            if response
                .get("output")
                .is_some_and(|output| !output.as_array().is_some_and(Vec::is_empty))
            {
                return Err(StaticEventCodecError::UnsupportedSemantics);
            }
            let response = response_id(required_string(response, "id")?, self.limits)?;
            let candidate = candidate_id("candidate_0", self.limits)?;
            self.response_id = Some(response.clone());
            self.candidate = Some(candidate.clone());
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::ResponseStarted {
                    response: ResponseIdentity::new(response),
                },
            )?);
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::CandidateStarted {
                    candidate: CandidateIdentity::new(candidate, OutputIndex::new(0)),
                },
            )?);
        } else if response.get("id").and_then(Value::as_str)
            != self.response_id.as_ref().map(|id| id.as_str())
        {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        let output = response
            .get("output")
            .map(|value| value.as_array().ok_or(StaticEventCodecError::InvalidJson))
            .transpose()?;
        if let Some(output) = output.filter(|output| !output.is_empty()) {
            if output.len() != self.items.len() {
                return Err(StaticEventCodecError::IdentityConflict);
            }
            for (index, snapshot) in output.iter().enumerate() {
                let item = self
                    .items
                    .get(&(index as u64))
                    .ok_or(StaticEventCodecError::IdentityConflict)?;
                let snapshot = snapshot
                    .as_object()
                    .ok_or(StaticEventCodecError::InvalidJson)?;
                let status = snapshot.get("status").filter(|value| !value.is_null());
                if !item.finished
                    || snapshot.get("id").and_then(Value::as_str) != Some(item.item.as_str())
                    || status.is_some_and(|value| value.as_str() != Some("completed"))
                    || (item.kind == ResponsesItemKind::Message && status.is_none())
                {
                    return Err(StaticEventCodecError::IdentityConflict);
                }
                validate_item_snapshot(item, snapshot)?;
                events.extend(self.annotation_events(&Value::Object(snapshot.clone()))?);
            }
        }
        let has_tools = self
            .items
            .values()
            .any(|item| item.kind == ResponsesItemKind::Tool);
        let candidate = self.candidate()?.clone();
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::CandidateFinished {
                candidate: CandidateRef::new(candidate),
                finish: if has_tools {
                    FinishReason::ToolCalls
                } else {
                    FinishReason::Stop
                },
            },
        )?);
        if let Some(usage) = response.get("usage").filter(|usage| !usage.is_null()) {
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::UsageSnapshot {
                    usage: usage_from_responses(
                        usage
                            .as_object()
                            .ok_or(StaticEventCodecError::InvalidJson)?,
                    )?,
                },
            )?);
        }
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(TerminalStatus::Completed, None),
            },
        )?);
        self.terminal = true;
        Ok(events)
    }

    fn non_completed(
        &mut self,
        value: &Map<String, Value>,
        status: TerminalStatus,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let response = value
            .get("response")
            .and_then(Value::as_object)
            .ok_or(StaticEventCodecError::InvalidJson)?;
        let expected_status = match status {
            TerminalStatus::Incomplete => "incomplete",
            TerminalStatus::Failed => "failed",
            TerminalStatus::Cancelled => "cancelled",
            _ => return Err(StaticEventCodecError::InvalidLifecycle),
        };
        if response.get("status").and_then(Value::as_str) != Some(expected_status) {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        if let Some(output) = response.get("output") {
            let output = output
                .as_array()
                .ok_or(StaticEventCodecError::InvalidJson)?;
            if !output.is_empty() {
                if output.len() != self.items.len() {
                    return Err(StaticEventCodecError::IdentityConflict);
                }
                for (index, snapshot) in output.iter().enumerate() {
                    let item = self
                        .items
                        .get(&(index as u64))
                        .ok_or(StaticEventCodecError::IdentityConflict)?;
                    let snapshot = snapshot
                        .as_object()
                        .ok_or(StaticEventCodecError::InvalidJson)?;
                    if snapshot.get("id").and_then(Value::as_str) != Some(item.item.as_str()) {
                        return Err(StaticEventCodecError::IdentityConflict);
                    }
                    validate_item_snapshot(item, snapshot)?;
                }
            }
        }
        let mut events = Vec::new();
        if self.response_id.is_none() {
            let id = required_string(response, "id")?;
            events.extend(
                self.created(
                    &json!({"response":{"id":id,"status":"in_progress","output":[]}})
                        .as_object()
                        .unwrap()
                        .clone(),
                )?,
            );
        }
        if response.get("id").and_then(Value::as_str)
            != self.response_id.as_ref().map(|id| id.as_str())
        {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        // Terminal metadata is retained as bounded IR extensions, not converted into readable text.
        for field in ["error", "incomplete_details"] {
            if let Some(details) = response.get(field).filter(|value| !value.is_null()) {
                if !details.is_object() {
                    return Err(StaticEventCodecError::InvalidJson);
                }
                let namespace = ProviderNamespace::new("openai.responses", 64)
                    .map_err(|_| StaticEventCodecError::LimitExceeded)?;
                let kind = ExtensionKind::new(format!("response.{field}"), 64)
                    .map_err(|_| StaticEventCodecError::LimitExceeded)?;
                let payload =
                    BoundedOpaqueJson::new(details.clone(), self.limits.max_event_bytes())
                        .map_err(|_| StaticEventCodecError::LimitExceeded)?;
                let extension =
                    ProviderExtension::new(namespace, kind, OpaquePayload::Json(payload), None)
                        .map_err(|_| StaticEventCodecError::InvalidJson)?;
                events.push(envelope(
                    &mut self.sequence,
                    GenerationEvent::Extension { extension },
                )?);
            }
        }
        if let Some(usage) = response.get("usage").and_then(Value::as_object) {
            events.push(envelope(
                &mut self.sequence,
                GenerationEvent::UsageSnapshot {
                    usage: usage_from_responses(usage)?,
                },
            )?);
        }
        self.terminal = true;
        events.push(envelope(
            &mut self.sequence,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(status, None),
            },
        )?);
        Ok(events)
    }

    fn error_terminal(&mut self) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        self.ensure_started()?;
        self.terminal = true;
        Ok(vec![envelope(
            &mut self.sequence,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(TerminalStatus::Error, None),
            },
        )?])
    }

    fn annotation_events(
        &mut self,
        item: &Value,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        let annotations =
            crate::bridge::static_codec::snapshot_annotations(item, self.limits.max_event_bytes())
                .map_err(|_| StaticEventCodecError::InvalidJson)?;
        if !self.native && !annotations.is_empty() {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        annotations
            .into_iter()
            .map(|extension| envelope(&mut self.sequence, GenerationEvent::Extension { extension }))
            .collect()
    }

    fn extension(
        &mut self,
        kind: &str,
        value: Map<String, Value>,
    ) -> Result<Vec<EventEnvelope>, StaticEventCodecError> {
        if !self.native {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        self.ensure_started()?;
        let namespace = ProviderNamespace::new("openai.responses", self.limits.max_event_bytes())
            .map_err(|_| StaticEventCodecError::LimitExceeded)?;
        let extension_kind =
            ExtensionKind::new(format!("event.{kind}"), self.limits.max_event_bytes())
                .map_err(|_| StaticEventCodecError::LimitExceeded)?;
        let payload = BoundedOpaqueJson::new(Value::Object(value), self.limits.max_event_bytes())
            .map_err(|_| StaticEventCodecError::LimitExceeded)?;
        let extension = ProviderExtension::new(
            namespace,
            extension_kind,
            OpaquePayload::Json(payload),
            None,
        )
        .map_err(|_| StaticEventCodecError::LimitExceeded)?;
        Ok(vec![envelope(
            &mut self.sequence,
            GenerationEvent::Extension { extension },
        )?])
    }

    fn bound_item(
        &self,
        value: &Map<String, Value>,
    ) -> Result<&ResponsesWireItem, StaticEventCodecError> {
        let index = required_u64(value, "output_index")?;
        let item = self
            .items
            .get(&index)
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        if item.finished || value.get("item_id").and_then(Value::as_str) != Some(item.item.as_str())
        {
            return Err(StaticEventCodecError::IdentityConflict);
        }
        Ok(item)
    }

    fn ensure_started(&self) -> Result<(), StaticEventCodecError> {
        if self.response_id.is_some() && self.candidate.is_some() {
            Ok(())
        } else {
            Err(StaticEventCodecError::InvalidLifecycle)
        }
    }

    fn candidate(&self) -> Result<&crate::ir::generation::CandidateId, StaticEventCodecError> {
        self.candidate
            .as_ref()
            .ok_or(StaticEventCodecError::InvalidLifecycle)
    }
}

fn validate_child_index(
    value: &Map<String, Value>,
    kind: PartKind,
) -> Result<(), StaticEventCodecError> {
    let field = match kind {
        PartKind::Text | PartKind::Refusal | PartKind::ReasoningText => "content_index",
        PartKind::ReasoningSummary => "summary_index",
        PartKind::ToolArguments => return Ok(()),
        PartKind::Opaque => return Err(StaticEventCodecError::UnsupportedSemantics),
    };
    if required_u64(value, field)? != 0 {
        return Err(StaticEventCodecError::IdentityConflict);
    }
    Ok(())
}

fn validate_item_snapshot(
    item: &ResponsesWireItem,
    snapshot: &Map<String, Value>,
) -> Result<(), StaticEventCodecError> {
    match item.kind {
        ResponsesItemKind::Message => {
            if item.part_kind.is_none()
                && snapshot
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            {
                return if snapshot.get("type").and_then(Value::as_str) == Some("message")
                    && snapshot.get("role").and_then(Value::as_str) == Some("assistant")
                {
                    Ok(())
                } else {
                    Err(StaticEventCodecError::IdentityConflict)
                };
            }
            let (kind, value) = snapshot_message(snapshot)?;
            if snapshot.get("type").and_then(Value::as_str) != Some("message")
                || snapshot.get("role").and_then(Value::as_str) != Some("assistant")
                || item.part_kind != Some(kind)
                || value != item.value
            {
                return Err(StaticEventCodecError::IdentityConflict);
            }
        }
        ResponsesItemKind::Reasoning => {
            let visible = snapshot_text(snapshot, "content", "reasoning_text")?;
            let summary = snapshot_text(snapshot, "summary", "summary_text")?;
            let expected = match item.part_kind {
                Some(PartKind::ReasoningText) => visible.as_str(),
                Some(PartKind::ReasoningSummary) => summary.as_str(),
                None if item.opaque.is_some() => "",
                _ => return Err(StaticEventCodecError::IdentityConflict),
            };
            if snapshot.get("type").and_then(Value::as_str) != Some("reasoning")
                || expected != item.value
                || snapshot
                    .get("encrypted_content")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    != item.opaque.as_deref()
            {
                return Err(StaticEventCodecError::IdentityConflict);
            }
        }
        ResponsesItemKind::Tool => {
            if snapshot.get("type").and_then(Value::as_str) != Some("function_call")
                || snapshot.get("call_id").and_then(Value::as_str)
                    != item.call_id.as_ref().map(|id| id.as_str())
                || snapshot.get("name").and_then(Value::as_str)
                    != item.tool_name.as_ref().map(|name| name.as_str())
                || snapshot.get("arguments").and_then(Value::as_str) != Some(item.value.as_str())
            {
                return Err(StaticEventCodecError::IdentityConflict);
            }
        }
    }
    Ok(())
}

fn snapshot_message(
    snapshot: &Map<String, Value>,
) -> Result<(PartKind, String), StaticEventCodecError> {
    let parts = snapshot
        .get("content")
        .and_then(Value::as_array)
        .ok_or(StaticEventCodecError::InvalidJson)?;
    if parts.len() != 1 {
        return Err(StaticEventCodecError::IdentityConflict);
    }
    let part = parts[0]
        .as_object()
        .ok_or(StaticEventCodecError::InvalidJson)?;
    match part.get("type").and_then(Value::as_str) {
        Some("output_text") => Ok((
            PartKind::Text,
            part.get("text")
                .and_then(Value::as_str)
                .ok_or(StaticEventCodecError::InvalidJson)?
                .to_owned(),
        )),
        Some("refusal") => Ok((
            PartKind::Refusal,
            part.get("refusal")
                .and_then(Value::as_str)
                .ok_or(StaticEventCodecError::InvalidJson)?
                .to_owned(),
        )),
        _ => Err(StaticEventCodecError::UnsupportedSemantics),
    }
}

fn snapshot_text(
    snapshot: &Map<String, Value>,
    field: &str,
    expected_type: &str,
) -> Result<String, StaticEventCodecError> {
    let Some(parts) = snapshot.get(field) else {
        return Ok(String::new());
    };
    let parts = parts.as_array().ok_or(StaticEventCodecError::InvalidJson)?;
    let mut text = String::new();
    for part in parts {
        let part = part.as_object().ok_or(StaticEventCodecError::InvalidJson)?;
        if part.get("type").and_then(Value::as_str) != Some(expected_type) {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        if let Some(annotations) = part.get("annotations") {
            let annotations = annotations
                .as_array()
                .ok_or(StaticEventCodecError::InvalidJson)?;
            if !annotations.is_empty() {
                return Err(StaticEventCodecError::UnsupportedSemantics);
            }
        }
        text.push_str(
            part.get("text")
                .and_then(Value::as_str)
                .ok_or(StaticEventCodecError::InvalidJson)?,
        );
    }
    Ok(text)
}

fn empty_array(object: &Map<String, Value>, field: &str) -> Result<bool, StaticEventCodecError> {
    match object.get(field) {
        None => Ok(true),
        Some(value) => value
            .as_array()
            .map(Vec::is_empty)
            .ok_or(StaticEventCodecError::InvalidJson),
    }
}

#[derive(Clone, Debug)]
struct EncodedResponsesItem {
    item: ItemId,
    index: u64,
    header: ItemHeader,
    part: Option<PartId>,
    part_kind: Option<PartKind>,
    value: String,
}

/// Canonical Event IR to Responses SSE encoder.
pub(super) struct ResponsesEventEncoder {
    limits: EventLimits,
    model: String,
    response_id: Option<String>,
    items: BTreeMap<ItemId, EncodedResponsesItem>,
    parts: BTreeMap<PartId, ItemId>,
    finish: Option<FinishReason>,
    usage: Option<Usage>,
    terminal: bool,
}

impl ResponsesEventEncoder {
    pub(super) fn new(limits: EventLimits, model: &str) -> Self {
        Self {
            limits,
            model: model.to_owned(),
            response_id: None,
            items: BTreeMap::new(),
            parts: BTreeMap::new(),
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
            GenerationEvent::ResponseStarted { response } => self.response_started(response),
            GenerationEvent::CandidateStarted { candidate } => {
                if candidate.index().get() != 0 {
                    return Err(StaticEventCodecError::UnsupportedSemantics);
                }
                Ok(Bytes::new())
            }
            GenerationEvent::ItemStarted { item, header, .. } => self.item_started(item, header),
            GenerationEvent::PartStarted { item, part, kind } => {
                self.part_started(item, part, *kind)
            }
            GenerationEvent::PartDelta { part, delta } => self.delta(part, delta),
            GenerationEvent::PartFinished { part } => self.part_finished(part),
            GenerationEvent::ItemFinished { item } => self.item_finished(item),
            GenerationEvent::CandidateFinished { finish, .. } => {
                self.finish = Some(finish.clone());
                Ok(Bytes::new())
            }
            GenerationEvent::UsageSnapshot { usage } => {
                self.usage = Some(*usage);
                Ok(Bytes::new())
            }
            GenerationEvent::Extension { .. } => Err(StaticEventCodecError::UnsupportedSemantics),
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

    fn response_started(
        &mut self,
        response: &ResponseIdentity,
    ) -> Result<Bytes, StaticEventCodecError> {
        if self.response_id.is_some() {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        self.response_id = Some(response.id().as_str().to_owned());
        self.responses_event(
            "response.created",
            json!({
                "response": {
                    "id": response.id().as_str(),
                    "model": self.model,
                    "object": "response",
                    "output": [],
                    "status": "in_progress"
                },
                "type": "response.created"
            }),
        )
    }

    fn item_started(
        &mut self,
        item: &ItemIdentity,
        header: &ItemHeader,
    ) -> Result<Bytes, StaticEventCodecError> {
        let snapshot = match header {
            ItemHeader::Message {
                role: MessageRole::Assistant,
            } => json!({
                "content": [], "id": item.id().as_str(), "role": "assistant",
                "status": "in_progress", "type": "message"
            }),
            ItemHeader::Message { .. } => {
                return Err(StaticEventCodecError::UnsupportedSemantics);
            }
            ItemHeader::Reasoning => json!({
                "content": [], "id": item.id().as_str(), "status": "in_progress",
                "summary": [], "type": "reasoning"
            }),
            ItemHeader::ToolCall { call, tool } => json!({
                "arguments": "", "call_id": call.as_str(), "id": item.id().as_str(),
                "name": tool.as_str(), "status": "in_progress", "type": "function_call"
            }),
        };
        self.items.insert(
            item.id().clone(),
            EncodedResponsesItem {
                item: item.id().clone(),
                index: item.index().get(),
                header: header.clone(),
                part: None,
                part_kind: None,
                value: String::new(),
            },
        );
        self.responses_event(
            "response.output_item.added",
            json!({"item": snapshot, "output_index": item.index().get(), "type": "response.output_item.added"}),
        )
    }

    fn part_started(
        &mut self,
        item: &ItemRef,
        part: &PartIdentity,
        kind: PartKind,
    ) -> Result<Bytes, StaticEventCodecError> {
        if kind == PartKind::Opaque {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        let item = self
            .items
            .get_mut(item.id())
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        if item.part.is_some() {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        item.part = Some(part.id().clone());
        item.part_kind = Some(kind);
        self.parts.insert(part.id().clone(), item.item.clone());
        Ok(Bytes::new())
    }

    fn delta(&mut self, part: &PartRef, delta: &PartDelta) -> Result<Bytes, StaticEventCodecError> {
        let item_id = self
            .parts
            .get(part.id())
            .cloned()
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        let (event, value) = match delta {
            PartDelta::Text(value) => ("response.output_text.delta", value),
            PartDelta::Refusal(value) => ("response.refusal.delta", value),
            PartDelta::ReasoningText(value) => ("response.reasoning_text.delta", value),
            PartDelta::ReasoningSummary(value) => ("response.reasoning_summary_text.delta", value),
            PartDelta::ToolArguments(value) => ("response.function_call_arguments.delta", value),
            PartDelta::Opaque(_) => return Err(StaticEventCodecError::UnsupportedSemantics),
        };
        let (item_value, output_index) = {
            let item = self.items.get_mut(&item_id).unwrap();
            item.value.push_str(value.as_str());
            (item.item.as_str().to_owned(), item.index)
        };
        let value = if matches!(delta, PartDelta::ToolArguments(_)) {
            json!({
                "delta": value.as_str(), "item_id": item_value,
                "output_index": output_index, "type": event
            })
        } else if matches!(delta, PartDelta::Refusal(_)) {
            json!({
                "content_index": 0, "delta": value.as_str(), "item_id": item_value,
                "output_index": output_index, "type": event
            })
        } else if matches!(delta, PartDelta::ReasoningSummary(_)) {
            json!({
                "delta": value.as_str(), "item_id": item_value,
                "output_index": output_index, "summary_index": 0, "type": event
            })
        } else {
            json!({
                "content_index": 0, "delta": value.as_str(), "item_id": item_value,
                "output_index": output_index, "type": event
            })
        };
        self.responses_event(event, value)
    }

    fn part_finished(&mut self, part: &PartRef) -> Result<Bytes, StaticEventCodecError> {
        let item_id = self
            .parts
            .get(part.id())
            .cloned()
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        let item = self.items.get(&item_id).unwrap();
        match item.part_kind {
            Some(PartKind::Text) => Ok(Bytes::new()),
            Some(PartKind::Refusal) => self.responses_event(
                "response.refusal.done",
                json!({
                    "content_index": 0, "item_id": item.item.as_str(),
                    "output_index": item.index, "refusal": item.value,
                    "type": "response.refusal.done"
                }),
            ),
            Some(PartKind::ReasoningText) => self.responses_event(
                "response.reasoning_text.done",
                json!({
                    "content_index": 0, "item_id": item.item.as_str(),
                    "output_index": item.index, "text": item.value,
                    "type": "response.reasoning_text.done"
                }),
            ),
            Some(PartKind::ReasoningSummary) => self.responses_event(
                "response.reasoning_summary_text.done",
                json!({
                    "summary_index": 0, "item_id": item.item.as_str(),
                    "output_index": item.index, "text": item.value,
                    "type": "response.reasoning_summary_text.done"
                }),
            ),
            Some(PartKind::ToolArguments) => self.responses_event(
                "response.function_call_arguments.done",
                json!({
                    "arguments": item.value, "item_id": item.item.as_str(),
                    "output_index": item.index, "type": "response.function_call_arguments.done"
                }),
            ),
            Some(PartKind::Opaque) => Err(StaticEventCodecError::UnsupportedSemantics),
            None => Err(StaticEventCodecError::InvalidLifecycle),
        }
    }

    fn item_finished(&mut self, item: &ItemRef) -> Result<Bytes, StaticEventCodecError> {
        let item = self
            .items
            .get(item.id())
            .ok_or(StaticEventCodecError::IdentityConflict)?;
        let snapshot = encoded_item(item, true)?;
        self.responses_event(
            "response.output_item.done",
            json!({"item": snapshot, "output_index": item.index, "type": "response.output_item.done"}),
        )
    }

    fn terminal(&mut self, terminal: &TurnTerminal) -> Result<Bytes, StaticEventCodecError> {
        if self.terminal {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        // Non-completed outcomes remain Native-only until a lossless Bridge contract is declared.
        if terminal.status() != TerminalStatus::Completed {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        if !matches!(
            self.finish,
            Some(FinishReason::Stop | FinishReason::ToolCalls)
        ) {
            return Err(StaticEventCodecError::UnsupportedSemantics);
        }
        let mut items = self.items.values().collect::<Vec<_>>();
        items.sort_by_key(|item| item.index);
        let output = items
            .into_iter()
            .map(|item| encoded_item(item, true))
            .collect::<Result<Vec<_>, _>>()?;
        let mut response = json!({
            "id": self.response_id(),
            "model": self.model,
            "object": "response",
            "output": output,
            "status": "completed"
        });
        if let Some(usage) = &self.usage {
            response["usage"] = encode_responses_usage(usage);
        }
        let bytes = self.responses_event(
            "response.completed",
            json!({"response": response, "type": "response.completed"}),
        )?;
        self.terminal = true;
        Ok(bytes)
    }

    fn responses_event(&self, event: &str, value: Value) -> Result<Bytes, StaticEventCodecError> {
        response_event(event, &value, self.limits)
    }

    fn response_id(&self) -> &str {
        self.response_id.as_deref().unwrap_or("response")
    }
}

fn encoded_item(
    item: &EncodedResponsesItem,
    completed: bool,
) -> Result<Value, StaticEventCodecError> {
    let status = if completed {
        "completed"
    } else {
        "in_progress"
    };
    Ok(match &item.header {
        ItemHeader::Message { .. } => {
            let (part_type, field) = match item.part_kind {
                Some(PartKind::Text) => ("output_text", "text"),
                Some(PartKind::Refusal) => ("refusal", "refusal"),
                _ => return Err(StaticEventCodecError::InvalidLifecycle),
            };
            let mut content = Map::new();
            content.insert("type".to_owned(), Value::String(part_type.to_owned()));
            content.insert(field.to_owned(), Value::String(item.value.clone()));
            if part_type == "output_text" {
                content.insert("annotations".to_owned(), json!([]));
            }
            json!({
                "content": [Value::Object(content)],
                "id": item.item.as_str(), "role": "assistant", "status": status, "type": "message"
            })
        }
        ItemHeader::Reasoning => {
            let (content, summary) = match item.part_kind {
                Some(PartKind::ReasoningText) => (
                    vec![json!({"text": item.value, "type": "reasoning_text"})],
                    Vec::new(),
                ),
                Some(PartKind::ReasoningSummary) => (
                    Vec::new(),
                    vec![json!({"text": item.value, "type": "summary_text"})],
                ),
                _ => return Err(StaticEventCodecError::InvalidLifecycle),
            };
            json!({
                "content": content, "id": item.item.as_str(), "status": status,
                "summary": summary, "type": "reasoning"
            })
        }
        ItemHeader::ToolCall { call, tool } => json!({
            "arguments": item.value, "call_id": call.as_str(), "id": item.item.as_str(),
            "name": tool.as_str(), "status": status, "type": "function_call"
        }),
    })
}

fn encode_responses_usage(usage: &Usage) -> Value {
    let mut value = json!({
        "input_tokens": usage.input_tokens(),
        "output_tokens": usage.output_tokens(),
        "total_tokens": usage.total_tokens()
    });
    if let Some(tokens) = usage.reasoning_tokens() {
        value["output_tokens_details"] = json!({"reasoning_tokens": tokens});
    }
    if let Some(tokens) = usage.cached_input_tokens() {
        value["input_tokens_details"] = json!({"cached_tokens": tokens});
    }
    value
}
