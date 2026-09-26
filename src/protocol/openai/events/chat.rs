//! Chat chunks use a single candidate and an explicit framing-level DONE boundary.
use super::*;
use serde_json::json;
impl EventDecoder {
    pub(super) fn chat(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        event_fields(o, &["id", "object", "created", "model", "choices", "usage"])?;
        if string(o, "object")? != "chat.completion.chunk" {
            return Err(CodecError::Invalid("chunk object"));
        }
        let mut out = vec![];
        if self.metadata.is_none() {
            self.emit(StreamEvent::Started, &mut out)?;
        }
        self.observe_metadata(o)?;
        let choices = o
            .get("choices")
            .and_then(Value::as_array)
            .ok_or(CodecError::Invalid("choices"))?;
        if self.chat_pending.is_some() {
            if !choices.is_empty() {
                return Err(CodecError::Invalid("chunk after finish"));
            }
            let usage = super::super::static_response::usage(o.get("usage"), Profile::Chat)?
                .ok_or(CodecError::Invalid("usage tail"))?;
            self.emit(StreamEvent::Usage(usage), &mut out)?;
            return Ok(out);
        }
        if choices.len() != 1 {
            return Err(CodecError::Unsupported("candidate count".into()));
        }
        if o.get("usage").is_some_and(|v| !v.is_null()) {
            return Err(CodecError::Invalid("early usage"));
        }
        let choice = object(&choices[0])?;
        fields(choice, &["index", "delta", "finish_reason", "logprobs"])?;
        if choice.get("index").and_then(Value::as_u64) != Some(0)
            || choice.get("logprobs").is_some_and(|v| !v.is_null())
        {
            return Err(CodecError::Unsupported("candidate".into()));
        }
        let delta = object(choice.get("delta").ok_or(CodecError::Invalid("delta"))?)?;
        fields(delta, &["role", "content", "refusal", "tool_calls"])?;
        if delta
            .get("role")
            .is_some_and(|v| v.as_str() != Some("assistant"))
        {
            return Err(CodecError::Invalid("role"));
        }
        // One Chat candidate owns an assistant message even when it only contains calls.
        // Establish the same grouping as static decoding before allocating call identities.
        let owner = if let Some(id) = self.chat_owner {
            id
        } else {
            let id = self.allocate_item()?;
            self.emit(
                StreamEvent::ItemStarted {
                    item: id,
                    kind: ItemKind::Message { phase: None },
                    replay: None,
                },
                &mut out,
            )?;
            self.chat_owner = Some(id);
            id
        };
        for (key, kind) in [("content", PartKind::Text), ("refusal", PartKind::Refusal)] {
            if let Some(v) = delta.get(key).filter(|v| !v.is_null()) {
                let fragment = v.as_str().ok_or(CodecError::Invalid("text delta"))?;
                let item = owner;
                let existing = self
                    .state()?
                    .item(item)?
                    .parts
                    .first()
                    .map(|p| (p.id, p.kind));
                let part = if let Some((part, old_kind)) = existing {
                    if kind != old_kind {
                        return Err(CodecError::Unsupported("mixed Chat text/refusal".into()));
                    }
                    part
                } else {
                    let id = self.allocate_part()?;
                    self.emit(
                        StreamEvent::PartStarted {
                            item,
                            part: id,
                            kind,
                        },
                        &mut out,
                    )?;
                    id
                };
                self.emit(
                    StreamEvent::Delta {
                        item,
                        part,
                        fragment: fragment.into(),
                        logprobs: vec![],
                    },
                    &mut out,
                )?;
            }
        }
        if let Some(calls) = delta.get("tool_calls").filter(|v| !v.is_null()) {
            for call in calls.as_array().ok_or(CodecError::Invalid("tool calls"))? {
                let call = object(call)?;
                fields(call, &["index", "id", "type", "function"])?;
                let n = index(call, "index")?;
                if call
                    .get("type")
                    .is_some_and(|v| v.as_str() != Some("function"))
                {
                    return Err(CodecError::Unsupported("tool kind".into()));
                }
                let f = object(
                    call.get("function")
                        .ok_or(CodecError::Invalid("function"))?,
                )?;
                fields(f, &["name", "arguments"])?;
                let item = if n == self.chat_calls.len() {
                    let item = self.allocate_item()?;
                    let kind = ItemKind::ToolCall {
                        call_id: text(string(call, "id")?, "call id", 256)?,
                        name: text(string(f, "name")?, "name", 128)?,
                        message: self.chat_owner,
                    };
                    self.emit(
                        StreamEvent::ItemStarted {
                            item,
                            kind,
                            replay: None,
                        },
                        &mut out,
                    )?;
                    let part = self.allocate_part()?;
                    self.emit(
                        StreamEvent::PartStarted {
                            item,
                            part,
                            kind: PartKind::Arguments,
                        },
                        &mut out,
                    )?;
                    self.chat_calls.push(item);
                    item
                } else {
                    *self
                        .chat_calls
                        .get(n)
                        .ok_or(CodecError::Invalid("tool index"))?
                };
                let ItemKind::ToolCall { call_id, name, .. } = &self.state()?.item(item)?.kind
                else {
                    return Err(CodecError::Invalid("call"));
                };
                if call
                    .get("id")
                    .is_some_and(|v| v.as_str() != Some(call_id.as_str()))
                    || f.get("name")
                        .is_some_and(|v| v.as_str() != Some(name.as_str()))
                {
                    return Err(CodecError::Invalid("call identity"));
                }
                if let Some(v) = f.get("arguments") {
                    let fragment = v.as_str().ok_or(CodecError::Invalid("arguments"))?;
                    let part = self.state()?.item(item)?.parts[0].id;
                    self.emit(
                        StreamEvent::Delta {
                            item,
                            part,
                            fragment: fragment.into(),
                            logprobs: vec![],
                        },
                        &mut out,
                    )?;
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").filter(|v| !v.is_null()) {
            let terminal = match reason.as_str() {
                Some("stop") if self.chat_calls.is_empty() => StreamTerminal::Completed,
                Some("tool_calls") if !self.chat_calls.is_empty() => StreamTerminal::Completed,
                Some("length") => StreamTerminal::Incomplete,
                _ => return Err(CodecError::Unsupported("finish reason".into())),
            };
            let items = self.state()?.items().to_vec();
            for item in items {
                for part in item.parts {
                    self.emit(
                        StreamEvent::ValueFinished {
                            item: item.id,
                            part: part.id,
                        },
                        &mut out,
                    )?;
                    self.emit(
                        StreamEvent::PartFinished {
                            item: item.id,
                            part: part.id,
                        },
                        &mut out,
                    )?;
                }
                self.emit(
                    StreamEvent::ItemFinished {
                        item: item.id,
                        status: if terminal == StreamTerminal::Completed {
                            ItemLifecycle::Completed
                        } else {
                            ItemLifecycle::Incomplete
                        },
                        replay: None,
                    },
                    &mut out,
                )?;
            }
            self.chat_pending = Some((
                terminal,
                if terminal == StreamTerminal::Incomplete {
                    TerminalDetails {
                        error: None,
                        incomplete: Some(IncompleteReason::MaxOutputTokens),
                    }
                } else {
                    TerminalDetails::default()
                },
            ));
        }
        Ok(out)
    }
}
impl EventEncoder {
    fn chunk(&self, delta: Value, finish: Value) -> Value {
        json!({"id":self.metadata.id,"object":"chat.completion.chunk","created":self.metadata.created,"model":self.metadata.model,"choices":[{"index":0,"delta":delta,"finish_reason":finish}],"usage":null})
    }
    fn call_index(&self, item: ItemId) -> Result<usize, CodecError> {
        self.state()?
            .items()
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::ToolCall { .. }))
            .position(|i| i.id == item)
            .ok_or(CodecError::Invalid("call index"))
    }
    pub(super) fn chat(&self, event: &StreamEvent) -> Result<Vec<Value>, CodecError> {
        Ok(match event {
            StreamEvent::Started=>vec![self.chunk(json!({"role":"assistant"}),Value::Null)],
            StreamEvent::ItemStarted{item,kind:ItemKind::ToolCall{call_id,name,..},..}=>vec![self.chunk(json!({"tool_calls":[{"index":self.call_index(*item)?,"id":call_id.as_str(),"type":"function","function":{"name":name.as_str(),"arguments":""}}]}),Value::Null)],
            StreamEvent::PartStarted{kind:PartKind::Text,..}=>vec![self.chunk(json!({"content":""}),Value::Null)],
            StreamEvent::PartStarted{kind:PartKind::Refusal,..}=>vec![self.chunk(json!({"refusal":""}),Value::Null)],
            StreamEvent::Delta{item,part,fragment,..}=>{
                let delta=match self.state()?.part(*item,*part)?.kind{PartKind::Text=>json!({"content":fragment}),PartKind::Refusal=>json!({"refusal":fragment}),PartKind::Arguments=>json!({"tool_calls":[{"index":self.call_index(*item)?,"function":{"arguments":fragment}}]}),_=>return Err(CodecError::Unsupported("Chat reasoning".into()))};vec![self.chunk(delta,Value::Null)]
            }
            StreamEvent::Terminal{terminal,..}=>{
                let response=materialize(self.state()?)?;
                crate::lowering::generation::lower_response(&response,&self.fidelity,&self.metadata,Profile::Chat,self.contract.clone()).map_err(|_|CodecError::Unsupported("Chat terminal".into()))?;
                let finish=match terminal{StreamTerminal::Incomplete=>"length",StreamTerminal::Completed=>if response.completion()==Some(Completion::ToolCalls){"tool_calls"}else{"stop"},_=>return Err(CodecError::Unsupported("Chat terminal".into()))};
                let mut chunks=vec![self.chunk(json!({}),json!(finish))];
                if let Some(usage)=response.usage(){let mut v=self.chunk(json!({}),Value::Null);v["choices"]=json!([]);v["usage"]=super::super::static_response::encode_usage(usage,Profile::Chat);chunks.push(v);}chunks
            }
            _=>vec![],
        })
    }
}
