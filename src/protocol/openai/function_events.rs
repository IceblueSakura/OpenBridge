//! Function-call event payloads. SSE framing and non-function domains are outside this slice.
use super::{
    CodecError, Profile, ResponseMetadata,
    common::{bounded, fields, object, string, text},
};
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::task::generation::{ItemId, MAX_ITEMS, MAX_TEXT_BYTES, StreamEvent, StreamTerminal},
};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrackedCall {
    item: ItemId,
    wire_id: Option<String>,
    call_id: String,
    name: String,
    arguments: String,
    finished: bool,
}
pub struct FunctionEventDecoder {
    profile: Profile,
    metadata: Option<ResponseMetadata>,
    fidelity: FidelityRecords,
    next_item: u64,
    message_owner: Option<ItemId>,
    terminal: bool,
    calls: BTreeMap<u64, TrackedCall>,
    created: bool,
}
impl FunctionEventDecoder {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            metadata: None,
            fidelity: FidelityRecords::default(),
            next_item: 0,
            message_owner: None,
            terminal: false,
            calls: BTreeMap::new(),
            created: false,
        }
    }
    pub fn push(&mut self, payload: &Value) -> Result<Vec<StreamEvent>, CodecError> {
        if self.terminal {
            return Err(CodecError::Invalid("event after terminal"));
        }
        bounded(payload)?;
        let object = object(payload)?;
        match self.profile {
            Profile::Chat => self.push_chat(object),
            Profile::Responses => self.push_responses(object),
        }
    }
    pub fn finish(&self) -> Result<(), CodecError> {
        if self.terminal {
            Ok(())
        } else {
            Err(CodecError::Invalid("eof before terminal"))
        }
    }
    pub fn into_parts(self) -> Result<(FidelityRecords, ResponseMetadata), CodecError> {
        self.metadata
            .map(|metadata| (self.fidelity, metadata))
            .ok_or(CodecError::Invalid("event metadata"))
    }
    fn push_chat(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["id", "object", "created", "model", "choices"])?;
        if string(o, "object")? != "chat.completion.chunk" {
            return Err(CodecError::Invalid("chunk object"));
        }
        self.observe_metadata(o, "created")?;
        let choices = o
            .get("choices")
            .and_then(Value::as_array)
            .filter(|choices| choices.len() == 1)
            .ok_or(CodecError::Unsupported("candidate count".into()))?;
        let choice = object(&choices[0])?;
        fields(choice, &["index", "delta", "finish_reason"])?;
        if choice.get("index").and_then(Value::as_u64) != Some(0) {
            return Err(CodecError::Unsupported("candidate index".into()));
        }
        let delta = object(choice.get("delta").ok_or(CodecError::Invalid("delta"))?)?;
        fields(delta, &["role", "content", "tool_calls"])?;
        if delta.contains_key("content") && !delta.get("content").is_some_and(Value::is_null) {
            return Err(CodecError::Unsupported("text delta".into()));
        }
        if let Some(role) = delta.get("role")
            && role.as_str() != Some("assistant")
        {
            return Err(CodecError::Unsupported("delta role".into()));
        }
        let mut events = Vec::new();
        if let Some(calls) = delta.get("tool_calls") {
            let calls = calls.as_array().ok_or(CodecError::Invalid("tool_calls"))?;
            for call in calls {
                events.extend(self.chat_call(object(call)?)?);
            }
        }
        match choice.get("finish_reason") {
            None | Some(Value::Null) => {
                if events.is_empty() && !delta.contains_key("role") {
                    return Err(CodecError::Invalid("empty chunk"));
                }
            }
            Some(reason) => {
                let terminal = match reason.as_str() {
                    Some("tool_calls") => StreamTerminal::Completed,
                    Some("length") => StreamTerminal::Incomplete,
                    _ => return Err(CodecError::Unsupported("finish reason".into())),
                };
                if terminal == StreamTerminal::Completed {
                    events.extend(self.finish_open_calls()?);
                }
                self.terminal = true;
                events.push(StreamEvent::Terminal(terminal));
            }
        }
        Ok(events)
    }
    fn chat_call(&mut self, call: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        fields(call, &["index", "id", "type", "function"])?;
        let index = call
            .get("index")
            .and_then(Value::as_u64)
            .ok_or(CodecError::Invalid("tool index"))?;
        if let Some(kind) = call.get("type")
            && kind.as_str() != Some("function")
        {
            return Err(CodecError::Unsupported("tool kind".into()));
        }
        let function = object(
            call.get("function")
                .ok_or(CodecError::Invalid("function"))?,
        )?;
        fields(function, &["name", "arguments"])?;
        let mut events = Vec::new();
        if !self.calls.contains_key(&index) && index != self.calls.len() as u64 {
            return Err(CodecError::Invalid("tool index"));
        }
        if !self.calls.contains_key(&index) {
            let call_id = string(call, "id")?;
            let name = string(function, "name")?;
            if self.calls.values().any(|known| known.call_id == call_id) {
                return Err(CodecError::Invalid("duplicate call id"));
            }
            let owner = self.message_owner()?;
            let item = self.item_id()?;
            self.calls.insert(
                index,
                TrackedCall {
                    item,
                    wire_id: None,
                    call_id: call_id.to_owned(),
                    name: name.to_owned(),
                    arguments: String::new(),
                    finished: false,
                },
            );
            events.push(StreamEvent::CallStarted {
                item,
                call_id: text(call_id, "call_id", 256)?,
                name: text(name, "function name", 128)?,
                message: Some(owner),
            });
        } else if call
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id != self.calls[&index].call_id)
            || function
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name != self.calls[&index].name)
        {
            return Err(CodecError::Invalid("event identity"));
        }
        if let Some(arguments) = function.get("arguments") {
            let arguments = arguments.as_str().ok_or(CodecError::Invalid("arguments"))?;
            if !arguments.is_empty() {
                let tracked = self.calls.get_mut(&index).expect("call was inserted");
                append(&mut tracked.arguments, arguments)?;
                events.push(StreamEvent::ArgumentsDelta {
                    item: tracked.item,
                    fragment: arguments.to_owned(),
                });
            }
        }
        Ok(events)
    }
    fn push_responses(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        let kind = string(o, "type")?;
        if kind == "error" {
            fields(o, &["type", "code", "message", "param"])?;
            text(string(o, "code")?, "error code", 128)?;
            text(string(o, "message")?, "error message", MAX_TEXT_BYTES)?;
            if let Some(param) = o.get("param")
                && !param.is_null()
            {
                return Err(CodecError::Unsupported("error param".into()));
            }
            self.terminal = true;
            return Ok(vec![StreamEvent::Terminal(StreamTerminal::Error)]);
        }
        if !self.created && kind != "response.created" {
            return Err(CodecError::Invalid("response not started"));
        }
        match kind {
            "response.created" | "response.in_progress" => self.responses_status(o, "in_progress"),
            "response.output_item.added" => self.item_added(o),
            "response.function_call_arguments.delta" => self.arguments_delta(o),
            "response.function_call_arguments.done" => self.arguments_done(o),
            "response.output_item.done" => self.item_done(o),
            "response.completed" => self.responses_terminal(o, StreamTerminal::Completed),
            "response.failed" => self.responses_terminal(o, StreamTerminal::Failed),
            "response.incomplete" => self.responses_terminal(o, StreamTerminal::Incomplete),
            _ => Err(CodecError::Unsupported(kind.into())),
        }
    }
    fn responses_status(
        &mut self,
        o: &Map<String, Value>,
        status: &str,
    ) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["type", "response"])?;
        let response = object(o.get("response").ok_or(CodecError::Invalid("response"))?)?;
        fields(
            response,
            &[
                "id",
                "object",
                "created_at",
                "model",
                "status",
                "output",
                "usage",
            ],
        )?;
        if response.get("status").and_then(Value::as_str) != Some(status) {
            return Err(CodecError::Invalid("response status"));
        }
        if response
            .get("output")
            .is_some_and(|output| output.as_array().is_none_or(|items| !items.is_empty()))
        {
            return Err(CodecError::Unsupported("early output".into()));
        }
        self.observe_metadata(response, "created_at")?;
        self.created = true;
        Ok(Vec::new())
    }
    fn item_added(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["type", "output_index", "item"])?;
        let index = index_of(o)?;
        if index != self.calls.len() as u64 {
            return Err(CodecError::Invalid("output index"));
        }
        let item = object(o.get("item").ok_or(CodecError::Invalid("item"))?)?;
        fields(
            item,
            &["id", "type", "call_id", "name", "arguments", "status"],
        )?;
        if string(item, "type")? != "function_call" || string(item, "status")? != "in_progress" {
            return Err(CodecError::Unsupported("output item".into()));
        }
        let wire_id = string(item, "id")?.to_owned();
        let call_id = string(item, "call_id")?;
        let name = string(item, "name")?;
        if self.calls.values().any(|call| call.call_id == call_id) {
            return Err(CodecError::Invalid("duplicate call id"));
        }
        let semantic_id = self.item_id()?;
        self.fidelity
            .record_response_item_id(semantic_id, &wire_id)?;
        let arguments = string(item, "arguments")?;
        self.calls.insert(
            index,
            TrackedCall {
                item: semantic_id,
                wire_id: Some(wire_id),
                call_id: call_id.to_owned(),
                name: name.to_owned(),
                arguments: String::new(),
                finished: false,
            },
        );
        let mut events = vec![StreamEvent::CallStarted {
            item: semantic_id,
            call_id: text(call_id, "call_id", 256)?,
            name: text(name, "function name", 128)?,
            message: None,
        }];
        if !arguments.is_empty() {
            append(
                &mut self.calls.get_mut(&index).expect("inserted").arguments,
                arguments,
            )?;
            events.push(StreamEvent::ArgumentsDelta {
                item: semantic_id,
                fragment: arguments.to_owned(),
            });
        }
        Ok(events)
    }
    fn arguments_delta(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["type", "output_index", "item_id", "delta"])?;
        let call = self.responses_call(o, false)?;
        let fragment = string(o, "delta")?;
        if fragment.is_empty() {
            return Err(CodecError::Invalid("empty delta"));
        }
        append(&mut call.arguments, fragment)?;
        Ok(vec![StreamEvent::ArgumentsDelta {
            item: call.item,
            fragment: fragment.to_owned(),
        }])
    }
    fn arguments_done(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["type", "output_index", "item_id", "arguments"])?;
        let call = self.responses_call(o, false)?;
        if string(o, "arguments")? != call.arguments {
            return Err(CodecError::Invalid("arguments snapshot"));
        }
        Ok(Vec::new())
    }
    fn item_done(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["type", "output_index", "item"])?;
        let index = index_of(o)?;
        let snapshot = object(o.get("item").ok_or(CodecError::Invalid("item"))?)?;
        fields(
            snapshot,
            &["id", "type", "call_id", "name", "arguments", "status"],
        )?;
        let call = self
            .calls
            .get_mut(&index)
            .filter(|call| !call.finished)
            .ok_or(CodecError::Invalid("event identity"))?;
        if string(snapshot, "id")? != call.wire_id.as_deref().unwrap_or_default()
            || string(snapshot, "type")? != "function_call"
            || string(snapshot, "status")? != "completed"
            || string(snapshot, "call_id")? != call.call_id
            || string(snapshot, "name")? != call.name
            || string(snapshot, "arguments")? != call.arguments
        {
            return Err(CodecError::Invalid("item snapshot"));
        }
        let item = call.item;
        call.finished = true;
        Ok(vec![StreamEvent::CallFinished { item }])
    }
    fn responses_terminal(
        &mut self,
        o: &Map<String, Value>,
        terminal: StreamTerminal,
    ) -> Result<Vec<StreamEvent>, CodecError> {
        fields(o, &["type", "response"])?;
        let response = object(o.get("response").ok_or(CodecError::Invalid("response"))?)?;
        fields(
            response,
            &[
                "id",
                "object",
                "created_at",
                "model",
                "status",
                "output",
                "usage",
                "error",
                "incomplete_details",
            ],
        )?;
        let expected = match terminal {
            StreamTerminal::Completed => "completed",
            StreamTerminal::Failed => "failed",
            StreamTerminal::Incomplete => "incomplete",
            StreamTerminal::Error => return Err(CodecError::Invalid("error terminal")),
        };
        if string(response, "status")? != expected {
            return Err(CodecError::Invalid("response status"));
        }
        self.observe_metadata(response, "created_at")?;
        if terminal == StreamTerminal::Completed {
            let output = response
                .get("output")
                .and_then(Value::as_array)
                .ok_or(CodecError::Invalid("output"))?;
            if output.len() != self.calls.len() || self.calls.values().any(|call| !call.finished) {
                return Err(CodecError::Invalid("incomplete output"));
            }
            for (index, item) in output.iter().enumerate() {
                let item = object(item)?;
                let call = self
                    .calls
                    .get(&(index as u64))
                    .ok_or(CodecError::Invalid("output index"))?;
                if string(item, "id")? != call.wire_id.as_deref().unwrap_or_default()
                    || string(item, "arguments")? != call.arguments
                    || string(item, "call_id")? != call.call_id
                {
                    return Err(CodecError::Invalid("completed snapshot"));
                }
            }
        }
        if response.get("usage").is_some_and(|usage| !usage.is_null()) {
            return Err(CodecError::Unsupported("event usage".into()));
        }
        self.terminal = true;
        Ok(vec![StreamEvent::Terminal(terminal)])
    }
    fn responses_call(
        &mut self,
        o: &Map<String, Value>,
        finished: bool,
    ) -> Result<&mut TrackedCall, CodecError> {
        let index = index_of(o)?;
        let item_id = string(o, "item_id")?;
        let call = self
            .calls
            .get_mut(&index)
            .filter(|call| call.finished == finished)
            .ok_or(CodecError::Invalid("event identity"))?;
        if call.wire_id.as_deref() != Some(item_id) {
            return Err(CodecError::Invalid("event identity"));
        }
        Ok(call)
    }
    fn finish_open_calls(&mut self) -> Result<Vec<StreamEvent>, CodecError> {
        let mut events = Vec::new();
        for call in self.calls.values_mut() {
            if call.finished {
                continue;
            }
            call.finished = true;
            events.push(StreamEvent::CallFinished { item: call.item });
        }
        if events.is_empty() {
            return Err(CodecError::Invalid("completion without calls"));
        }
        Ok(events)
    }
    fn message_owner(&mut self) -> Result<ItemId, CodecError> {
        if let Some(owner) = self.message_owner {
            return Ok(owner);
        }
        let owner = self.item_id()?;
        self.message_owner = Some(owner);
        Ok(owner)
    }
    fn item_id(&mut self) -> Result<ItemId, CodecError> {
        if self.next_item as usize >= MAX_ITEMS {
            return Err(CodecError::Limit);
        }
        self.next_item += 1;
        Ok(ItemId::new(self.next_item))
    }
    fn observe_metadata(
        &mut self,
        o: &Map<String, Value>,
        created_key: &str,
    ) -> Result<(), CodecError> {
        let id = string(o, "id")?;
        let model = string(o, "model")?;
        let created = o
            .get(created_key)
            .and_then(Value::as_u64)
            .ok_or(CodecError::Invalid("created time"))?;
        let next = ResponseMetadata {
            id: text(id, "response id", 256)?.as_str().to_owned(),
            model: text(model, "response model", 256)?.as_str().to_owned(),
            created,
            usage: None,
        };
        if let Some(current) = &self.metadata
            && current != &next
        {
            return Err(CodecError::Invalid("event identity"));
        }
        self.metadata = Some(next);
        Ok(())
    }
}
pub struct FunctionEventEncoder {
    profile: Profile,
    metadata: ResponseMetadata,
    calls: Vec<TrackedCall>,
    next_index: u64,
}
impl FunctionEventEncoder {
    pub fn new(profile: Profile, metadata: ResponseMetadata) -> Result<Self, CodecError> {
        if metadata.id.is_empty()
            || metadata.model.is_empty()
            || metadata.id.len() > 256
            || metadata.model.len() > 256
            || metadata.usage.is_some()
        {
            return Err(CodecError::Invalid("event metadata"));
        }
        Ok(Self {
            profile,
            metadata,
            calls: Vec::new(),
            next_index: 0,
        })
    }
    pub fn encode(
        &mut self,
        event: &StreamEvent,
        fidelity: &FidelityRecords,
    ) -> Result<Vec<Value>, CodecError> {
        match self.profile {
            Profile::Chat => self.encode_chat(event),
            Profile::Responses => self.encode_responses(event, fidelity),
        }
    }
    fn encode_chat(&mut self, event: &StreamEvent) -> Result<Vec<Value>, CodecError> {
        match event {
            StreamEvent::CallStarted {
                item,
                call_id,
                name,
                ..
            } => {
                let first = self.calls.is_empty();
                let index = self.remember(*item, None, call_id.as_str(), name.as_str())?;
                let mut delta = json!({"tool_calls":[{"index":index,"id":call_id.as_str(),"type":"function","function":{"name":name.as_str(),"arguments":""}}]});
                if first {
                    delta["role"] = json!("assistant");
                }
                Ok(vec![self.chat_chunk(delta, Value::Null)])
            }
            StreamEvent::ArgumentsDelta { item, fragment } => {
                let index = self.index_of(*item)?;
                let call = self.open_encoded(*item)?;
                append(&mut call.arguments, fragment)?;
                Ok(vec![self.chat_chunk(
                    json!({"tool_calls":[{"index":index,"function":{"arguments":fragment}}]}),
                    Value::Null,
                )])
            }
            StreamEvent::CallFinished { item } => {
                self.open_encoded(*item)?.finished = true;
                Ok(Vec::new())
            }
            StreamEvent::Terminal(StreamTerminal::Completed) => {
                if self.calls.is_empty() || self.calls.iter().any(|call| !call.finished) {
                    return Err(CodecError::Invalid("event lifecycle"));
                }
                Ok(vec![self.chat_chunk(json!({}), json!("tool_calls"))])
            }
            StreamEvent::Terminal(StreamTerminal::Incomplete) => {
                Ok(vec![self.chat_chunk(json!({}), json!("length"))])
            }
            StreamEvent::Terminal(_) => Err(CodecError::Unsupported("chat terminal".into())),
        }
    }
    fn encode_responses(
        &mut self,
        event: &StreamEvent,
        fidelity: &FidelityRecords,
    ) -> Result<Vec<Value>, CodecError> {
        match event {
            StreamEvent::CallStarted {
                item,
                call_id,
                name,
                ..
            } => {
                let wire_id = fidelity
                    .response_item_id(*item)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("item_{}", item.get()));
                let index = self.remember(
                    *item,
                    Some(wire_id.clone()),
                    call_id.as_str(),
                    name.as_str(),
                )?;
                Ok(vec![
                    json!({"type":"response.output_item.added","output_index":index,"item":{"id":wire_id,"type":"function_call","call_id":call_id.as_str(),"name":name.as_str(),"arguments":"","status":"in_progress"}}),
                ])
            }
            StreamEvent::ArgumentsDelta { item, fragment } => {
                let index = self.index_of(*item)?;
                let call = self.open_encoded(*item)?;
                append(&mut call.arguments, fragment)?;
                let wire_id = call.wire_id.clone().expect("responses call has wire id");
                Ok(vec![
                    json!({"type":"response.function_call_arguments.delta","output_index":index,"item_id":wire_id,"delta":fragment}),
                ])
            }
            StreamEvent::CallFinished { item } => {
                let index = self.index_of(*item)?;
                let call = self.open_encoded(*item)?;
                call.finished = true;
                let wire_id = call.wire_id.clone().expect("responses call has wire id");
                let arguments = call.arguments.clone();
                let call_id = call.call_id.clone();
                let name = call.name.clone();
                Ok(vec![
                    json!({"type":"response.function_call_arguments.done","output_index":index,"item_id":&wire_id,"arguments":&arguments}),
                    json!({"type":"response.output_item.done","output_index":index,"item":{"id":wire_id,"type":"function_call","call_id":call_id,"name":name,"arguments":arguments,"status":"completed"}}),
                ])
            }
            StreamEvent::Terminal(StreamTerminal::Completed) => {
                if self.calls.is_empty() || self.calls.iter().any(|call| !call.finished) {
                    return Err(CodecError::Invalid("event lifecycle"));
                }
                let output: Vec<_> = self
                    .calls
                    .iter()
                    .map(|call| {
                        json!({"id":call.wire_id,"type":"function_call","call_id":call.call_id,"name":call.name,"arguments":call.arguments,"status":"completed"})
                    })
                    .collect();
                Ok(vec![self.responses_terminal(
                    "response.completed",
                    "completed",
                    Some(output),
                )])
            }
            StreamEvent::Terminal(terminal) => {
                let (kind, status) = match terminal {
                    StreamTerminal::Failed => ("response.failed", "failed"),
                    StreamTerminal::Incomplete => ("response.incomplete", "incomplete"),
                    StreamTerminal::Error => {
                        return Ok(vec![
                            json!({"type":"error","code":"server_error","message":"error","param":null}),
                        ]);
                    }
                    StreamTerminal::Completed => unreachable!("completed is handled above"),
                };
                Ok(vec![self.responses_terminal(kind, status, None)])
            }
        }
    }
    fn remember(
        &mut self,
        item: ItemId,
        wire_id: Option<String>,
        call_id: &str,
        name: &str,
    ) -> Result<u64, CodecError> {
        if self.calls.iter().any(|call| call.item == item) || self.next_index as usize >= MAX_ITEMS
        {
            return Err(CodecError::Invalid("event identity"));
        }
        let index = self.next_index;
        self.next_index += 1;
        self.calls.push(TrackedCall {
            item,
            wire_id,
            call_id: call_id.to_owned(),
            name: name.to_owned(),
            arguments: String::new(),
            finished: false,
        });
        Ok(index)
    }
    fn open_encoded(&mut self, item: ItemId) -> Result<&mut TrackedCall, CodecError> {
        self.calls
            .iter_mut()
            .find(|call| call.item == item && !call.finished)
            .ok_or(CodecError::Invalid("event identity"))
    }
    fn index_of(&self, item: ItemId) -> Result<u64, CodecError> {
        self.calls
            .iter()
            .position(|call| call.item == item)
            .map(|index| index as u64)
            .ok_or(CodecError::Invalid("event identity"))
    }
    fn chat_chunk(&self, delta: Value, finish_reason: Value) -> Value {
        json!({"id":self.metadata.id,"object":"chat.completion.chunk","created":self.metadata.created,"model":self.metadata.model,"choices":[{"index":0,"delta":delta,"finish_reason":finish_reason}]})
    }
    fn responses_terminal(&self, kind: &str, status: &str, output: Option<Vec<Value>>) -> Value {
        json!({"type":kind,"response":{"id":self.metadata.id,"object":"response","created_at":self.metadata.created,"model":self.metadata.model,"status":status,"output":output.unwrap_or_default()}})
    }
}
fn index_of(o: &Map<String, Value>) -> Result<u64, CodecError> {
    o.get("output_index")
        .and_then(Value::as_u64)
        .filter(|index| *index < MAX_ITEMS as u64)
        .ok_or(CodecError::Invalid("output index"))
}
fn append(arguments: &mut String, fragment: &str) -> Result<(), CodecError> {
    if arguments.len() + fragment.len() > MAX_TEXT_BYTES {
        return Err(CodecError::Limit);
    }
    arguments.push_str(fragment);
    Ok(())
}
