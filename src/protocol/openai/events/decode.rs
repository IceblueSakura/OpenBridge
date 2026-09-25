//! Responses payload grammar over the shared semantic reducer.
use super::*;
use serde_json::json;

pub struct EventDecoder {
    pub(super) profile: Profile,
    pub(super) state: Option<StreamState>,
    pub(super) fidelity: FidelityRecords,
    pub(super) metadata: Option<ResponseMetadata>,
    pub(super) origin: Option<ReplayOrigin>,
    pub(super) ids: Vec<ItemId>,
    pub(super) next_part: u64,
    pub(super) poisoned: bool,
    sequence: Option<u64>,
    events: usize,
    pub(super) chat_pending: Option<(StreamTerminal, TerminalDetails)>,
    pub(super) chat_owner: Option<ItemId>,
    pub(super) chat_calls: Vec<ItemId>,
}
impl EventDecoder {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            state: Some(StreamState::new()),
            fidelity: FidelityRecords::default(),
            metadata: None,
            origin: None,
            ids: vec![],
            next_part: 0,
            poisoned: false,
            sequence: None,
            events: 0,
            chat_pending: None,
            chat_owner: None,
            chat_calls: vec![],
        }
    }
    pub fn with_replay_origin(mut self, origin: ReplayOrigin) -> Self {
        self.origin = Some(origin);
        self
    }
    pub fn push(&mut self, payload: &Value) -> Result<Vec<StreamEvent>, CodecError> {
        if self.poisoned || self.state.as_ref().is_none_or(|s| s.terminal().is_some()) {
            return Err(CodecError::Invalid("event after terminal or rejection"));
        }
        self.events += 1;
        let result = (|| {
            if self.events > 1_000_000 {
                return Err(CodecError::Limit);
            }
            bounded(payload)?;
            let o = object(payload)?;
            if let Some(padding) = o.get("obfuscation")
                && (self.profile != Profile::Responses
                    || !string(o, "type")?.ends_with(".delta")
                    || !padding.as_str().is_some_and(|s| s.len() <= 4096))
            {
                return Err(CodecError::Invalid("obfuscation"));
            }
            if let Some(n) = o.get("sequence_number") {
                let n = n.as_u64().ok_or(CodecError::Invalid("sequence"))?;
                if self.sequence.is_some_and(|old| n <= old) {
                    return Err(CodecError::Invalid("sequence"));
                }
                self.sequence = Some(n);
            }
            if self.profile == Profile::Responses {
                self.responses(o)
            } else {
                self.chat(o)
            }
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// Available immediately after response.created; no whole-stream buffering is required.
    pub fn metadata(&self) -> Option<&ResponseMetadata> {
        self.metadata.as_ref()
    }
    pub fn fidelity(&self) -> &FidelityRecords {
        &self.fidelity
    }
    pub fn finish(&self) -> Result<(), CodecError> {
        if self.poisoned {
            return Err(CodecError::Invalid("rejected stream"));
        }
        end_of_stream(
            self.state
                .as_ref()
                .ok_or(CodecError::Invalid("stream state"))?,
        )?;
        Ok(())
    }
    /// Framing calls this only for a real Chat `[DONE]`, never for transport EOF.
    pub fn done(&mut self) -> Result<Vec<StreamEvent>, CodecError> {
        if self.profile != Profile::Chat || self.poisoned {
            return Err(CodecError::Invalid("DONE"));
        }
        let Some((terminal, details)) = self.chat_pending.take() else {
            self.poisoned = true;
            return Err(CodecError::Invalid("DONE before finish"));
        };
        let mut out = vec![];
        self.emit(StreamEvent::Terminal { terminal, details }, &mut out)?;
        Ok(out)
    }
    pub fn materialize(&self) -> Result<super::super::DecodedResponse, CodecError> {
        self.finish()?;
        Ok(super::super::DecodedResponse {
            semantic: materialize(self.state.as_ref().ok_or(CodecError::Invalid("state"))?)?,
            fidelity: self.fidelity.clone(),
            metadata: self
                .metadata
                .clone()
                .ok_or(CodecError::Invalid("metadata"))?,
        })
    }
    pub fn into_parts(self) -> Result<(FidelityRecords, ResponseMetadata), CodecError> {
        self.finish()?;
        Ok((
            self.fidelity,
            self.metadata.ok_or(CodecError::Invalid("metadata"))?,
        ))
    }
    pub(super) fn state(&self) -> Result<&StreamState, CodecError> {
        self.state
            .as_ref()
            .ok_or(CodecError::Invalid("rejected stream"))
    }
    pub(super) fn emit(
        &mut self,
        event: StreamEvent,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        self.state = Some(reduce(
            self.state.take().ok_or(CodecError::Invalid("state"))?,
            event.clone(),
        )?);
        if let StreamEvent::ItemStarted { item, .. } | StreamEvent::ItemFinished { item, .. } =
            &event
        {
            sync_replays(
                self.state.as_ref().ok_or(CodecError::Invalid("state"))?,
                &mut self.fidelity,
                Some(*item),
            )?;
        }
        out.push(event);
        Ok(())
    }
    pub(super) fn allocate_item(&mut self) -> Result<ItemId, CodecError> {
        if self.ids.len() >= MAX_ITEMS {
            return Err(CodecError::Limit);
        }
        let id = ItemId::new(self.ids.len() as u64 + 1);
        self.ids.push(id);
        Ok(id)
    }
    pub(super) fn allocate_part(&mut self) -> Result<PartId, CodecError> {
        if self.next_part >= MAX_ITEMS as u64 {
            return Err(CodecError::Limit);
        }
        self.next_part += 1;
        Ok(PartId::new(self.next_part))
    }
    pub(super) fn observe_metadata(&mut self, o: &Map<String, Value>) -> Result<(), CodecError> {
        let m = super::super::static_response::metadata(o, self.profile)?;
        if self
            .metadata
            .as_ref()
            .is_some_and(|old| old.id != m.id || old.model != m.model || old.created != m.created)
        {
            return Err(CodecError::Invalid("metadata changed"));
        }
        self.metadata = Some(m);
        Ok(())
    }
    fn responses(&mut self, o: &Map<String, Value>) -> Result<Vec<StreamEvent>, CodecError> {
        let mut out = vec![];
        let typ = string(o, "type")?;
        match typ {
            "error" => {
                event_fields(o, &["type", "code", "message", "param"])?;
                let mut e = o.clone();
                e.remove("type");
                e.remove("sequence_number");
                let details = super::super::terminal::decode_details(
                    &serde_json::from_value(json!({"error":e}))
                        .map_err(|_| CodecError::Invalid("error"))?,
                )?;
                self.emit(
                    StreamEvent::Terminal {
                        terminal: StreamTerminal::Error,
                        details,
                    },
                    &mut out,
                )?;
            }
            "response.created" | "response.in_progress" => {
                event_fields(o, &["type", "response"])?;
                let r = object(o.get("response").ok_or(CodecError::Invalid("response"))?)?;
                super::super::envelope::response_fields(r)?;
                if string(r, "object")? != "response"
                    || string(r, "status")? != "in_progress"
                    || r.get("output")
                        .is_some_and(|v| !v.as_array().is_some_and(Vec::is_empty))
                    || ["usage", "error", "incomplete_details"]
                        .iter()
                        .any(|k| r.get(*k).is_some_and(|v| !v.is_null()))
                {
                    return Err(CodecError::Invalid("initial response"));
                }
                if typ == "response.created" {
                    self.emit(StreamEvent::Started, &mut out)?;
                } else if self.metadata.is_none() {
                    return Err(CodecError::Invalid("response not started"));
                }
                self.observe_metadata(r)?;
            }
            "response.output_item.added" => self.item_added(o, &mut out)?,
            "response.output_text.annotation.added" => self.annotation_added(o, &mut out)?,
            "response.content_part.added" | "response.reasoning_summary_part.added" => {
                self.part_added(o, &mut out)?
            }
            "response.output_text.delta"
            | "response.refusal.delta"
            | "response.reasoning_summary_text.delta"
            | "response.reasoning_text.delta"
            | "response.function_call_arguments.delta"
            | "response.custom_tool_call_input.delta" => self.delta(o, &mut out)?,
            "response.output_text.done"
            | "response.refusal.done"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_text.done"
            | "response.function_call_arguments.done"
            | "response.custom_tool_call_input.done" => self.value_done(o, &mut out)?,
            "response.content_part.done" | "response.reasoning_summary_part.done" => {
                self.part_done(o, &mut out)?
            }
            "response.output_item.done" => self.item_done(o, &mut out)?,
            "response.completed"
            | "response.incomplete"
            | "response.failed"
            | "response.cancelled" => self.terminal(o, &mut out)?,
            _ => return Err(CodecError::Unsupported(typ.into())),
        }
        Ok(out)
    }
    fn item_added(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(o, &["type", "output_index", "item"])?;
        if index(o, "output_index")? != self.ids.len() {
            return Err(CodecError::Invalid("output index"));
        }
        let v = object(o.get("item").ok_or(CodecError::Invalid("item"))?)?;
        if v.get("status")
            .filter(|v| !v.is_null())
            .is_some_and(|s| s.as_str() != Some("in_progress"))
        {
            return Err(CodecError::Invalid("item status"));
        }
        let kind = match string(v, "type")? {
            "message" => {
                fields(v, &["type", "id", "role", "status", "content"])?;
                if string(v, "role")? != "assistant"
                    || !v
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty)
                {
                    return Err(CodecError::Invalid("initial message"));
                }
                ItemKind::Message
            }
            "reasoning" => {
                fields(
                    v,
                    &[
                        "type",
                        "id",
                        "status",
                        "summary",
                        "content",
                        "encrypted_content",
                    ],
                )?;
                if !v
                    .get("summary")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
                    || v.get("content")
                        .is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
                {
                    return Err(CodecError::Invalid("initial reasoning"));
                }
                ItemKind::Reasoning
            }
            "custom_tool_call" => {
                fields(
                    v,
                    &[
                        "type",
                        "id",
                        "call_id",
                        "name",
                        "input",
                        "namespace",
                        "caller",
                        "async",
                    ],
                )?;
                super::super::responses::direct(v)?;
                ItemKind::CustomCall {
                    call_id: text(string(v, "call_id")?, "call id", 256)?,
                    name: text(string(v, "name")?, "custom name", 128)?,
                }
            }
            "function_call" => {
                fields(
                    v,
                    &[
                        "type",
                        "id",
                        "status",
                        "call_id",
                        "name",
                        "arguments",
                        "caller",
                        "namespace",
                        "async",
                    ],
                )?;
                super::super::responses::direct(v)?;
                ItemKind::ToolCall {
                    call_id: text(string(v, "call_id")?, "call id", 256)?,
                    name: text(string(v, "name")?, "tool name", 128)?,
                    message: None,
                }
            }
            _ => return Err(CodecError::Unsupported("item kind".into())),
        };
        let item = self.allocate_item()?;
        self.fidelity
            .record_response_item_id(item, string(v, "id")?)?;
        let replay = if matches!(kind, ItemKind::Reasoning) {
            replay(v, self.origin.as_ref(), false)?
        } else {
            None
        };
        self.emit(
            StreamEvent::ItemStarted {
                item,
                kind: kind.clone(),
                replay,
            },
            out,
        )?;
        if kind.call().is_some() {
            let part_kind = if matches!(kind, ItemKind::CustomCall { .. }) {
                PartKind::CustomInput
            } else {
                PartKind::Arguments
            };
            let part = self.allocate_part()?;
            self.emit(
                StreamEvent::PartStarted {
                    item,
                    part,
                    kind: part_kind,
                },
                out,
            )?;
            let fragment = string(
                v,
                if part_kind == PartKind::CustomInput {
                    "input"
                } else {
                    "arguments"
                },
            )?;
            if !fragment.is_empty() {
                self.emit(
                    StreamEvent::Delta {
                        item,
                        part,
                        fragment: fragment.into(),
                        logprobs: vec![],
                    },
                    out,
                )?;
            }
        }
        Ok(())
    }
    fn owner(&self, o: &Map<String, Value>) -> Result<ItemId, CodecError> {
        let id = *self
            .ids
            .get(index(o, "output_index")?)
            .ok_or(CodecError::Invalid("output index"))?;
        if string(o, "item_id")? != self.fidelity.response_item_id(id).unwrap_or("") {
            return Err(CodecError::Invalid("item id"));
        }
        Ok(id)
    }
    fn part_for(&self, item: ItemId, kind: PartKind, n: usize) -> Result<PartId, CodecError> {
        // Responses summary and content indexes have independent namespaces.
        self.state()?
            .item(item)?
            .parts
            .iter()
            .filter(|p| {
                if kind == PartKind::Summary {
                    p.kind == PartKind::Summary
                } else {
                    p.kind != PartKind::Summary
                }
            })
            .nth(n)
            .filter(|p| p.kind == kind)
            .map(|p| p.id)
            .ok_or(CodecError::Invalid("part index"))
    }
    fn annotation_added(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(
            o,
            &[
                "type",
                "item_id",
                "output_index",
                "content_index",
                "annotation_index",
                "annotation",
            ],
        )?;
        let item = self.owner(o)?;
        let part = self.part_for(item, PartKind::Text, index(o, "content_index")?)?;
        if index(o, "annotation_index")? != self.state()?.part(item, part)?.annotations.len() {
            return Err(CodecError::Invalid("annotation index"));
        }
        let v = o
            .get("annotation")
            .ok_or(CodecError::Invalid("annotation"))?;
        let annotation: Annotation =
            serde_json::from_value(v.clone()).map_err(|_| CodecError::Invalid("annotation"))?;
        if matches!(annotation, Annotation::FilePath { .. }) {
            return Err(CodecError::Unsupported("file_path annotation event".into()));
        }
        self.emit(
            StreamEvent::AnnotationAdded {
                item,
                part,
                annotation,
            },
            out,
        )?;
        Ok(())
    }
    fn part_added(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(
            o,
            &[
                "type",
                "output_index",
                "item_id",
                "content_index",
                "summary_index",
                "part",
            ],
        )?;
        let item = self.owner(o)?;
        let p = object(o.get("part").ok_or(CodecError::Invalid("part"))?)?;
        let kind = part_kind(p)?;
        let summary = string(o, "type")? == "response.reasoning_summary_part.added";
        if o.contains_key(if summary {
            "content_index"
        } else {
            "summary_index"
        }) {
            return Err(CodecError::Invalid("part index domain"));
        }
        if summary != (kind == PartKind::Summary) {
            return Err(CodecError::Invalid("part kind"));
        }
        let key = if summary {
            "summary_index"
        } else {
            "content_index"
        };
        let n = index(o, key)?;
        let count = self
            .state()?
            .item(item)?
            .parts
            .iter()
            .filter(|p| (p.kind == PartKind::Summary) == summary)
            .count();
        if n != count || !part_text(p, kind)?.is_empty() {
            return Err(CodecError::Invalid("initial part"));
        }
        if kind == PartKind::Text {
            let text = super::super::text::read(p, false)?;
            if !text.annotations().is_empty()
                || text.logprobs().value().is_some_and(|v| !v.is_empty())
            {
                return Err(CodecError::Invalid("initial text metadata"));
            }
        }
        let part = self.allocate_part()?;
        self.emit(StreamEvent::PartStarted { item, part, kind }, out)
    }
    fn value_identity(
        &mut self,
        o: &Map<String, Value>,
        allow_implicit: bool,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(ItemId, PartId, PartKind), CodecError> {
        let item = self.owner(o)?;
        let t = string(o, "type")?;
        let (kind, key) = if t.starts_with("response.custom_tool_call_input.") {
            (PartKind::CustomInput, None)
        } else if t.starts_with("response.function_call_arguments.") {
            (PartKind::Arguments, None)
        } else if t.starts_with("response.reasoning_summary_text.") {
            (PartKind::Summary, Some("summary_index"))
        } else if t.starts_with("response.reasoning_text.") {
            (PartKind::ReasoningText, Some("content_index"))
        } else if t.starts_with("response.refusal.") {
            (PartKind::Refusal, Some("content_index"))
        } else {
            (PartKind::Text, Some("content_index"))
        };
        if ["content_index", "summary_index"]
            .iter()
            .any(|k| Some(*k) != key && o.contains_key(*k))
        {
            return Err(CodecError::Invalid("part index domain"));
        }
        let n = key.map(|k| index(o, k)).transpose()?.unwrap_or(0);
        let part = match self.part_for(item, kind, n) {
            Ok(p) => p,
            Err(_)
                if allow_implicit
                    && kind == PartKind::ReasoningText
                    && n == self
                        .state()?
                        .item(item)?
                        .parts
                        .iter()
                        .filter(|p| p.kind != PartKind::Summary)
                        .count() =>
            {
                let p = self.allocate_part()?;
                self.emit(
                    StreamEvent::PartStarted {
                        item,
                        part: p,
                        kind,
                    },
                    out,
                )?;
                p
            }
            Err(e) => return Err(e),
        };
        Ok((item, part, kind))
    }
    fn delta(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(
            o,
            &[
                "type",
                "output_index",
                "item_id",
                "content_index",
                "summary_index",
                "delta",
                "logprobs",
            ],
        )?;
        let (item, part, kind) = self.value_identity(o, true, out)?;
        if o.contains_key("logprobs") && kind != PartKind::Text {
            return Err(CodecError::Invalid("logprob owner"));
        }
        let logprobs = o
            .get("logprobs")
            .map(super::super::text::read_logprobs)
            .transpose()?
            .unwrap_or_default();
        self.emit(
            StreamEvent::Delta {
                item,
                part,
                fragment: string(o, "delta")?.into(),
                logprobs,
            },
            out,
        )
    }
    fn value_done(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(
            o,
            &[
                "type",
                "output_index",
                "item_id",
                "content_index",
                "summary_index",
                "text",
                "refusal",
                "arguments",
                "input",
                "logprobs",
            ],
        )?;
        let (item, part, kind) = self.value_identity(o, true, out)?;
        let key = match kind {
            PartKind::Arguments => "arguments",
            PartKind::CustomInput => "input",
            PartKind::Refusal => "refusal",
            _ => "text",
        };
        if ["text", "refusal", "arguments", "input"]
            .iter()
            .any(|k| *k != key && o.contains_key(*k))
        {
            return Err(CodecError::Invalid("value domain"));
        }
        if string(o, key)? != self.state()?.part(item, part)?.text {
            return Err(CodecError::Invalid("value snapshot"));
        }
        if let Some(v) = o.get("logprobs") {
            if kind != PartKind::Text {
                return Err(CodecError::Invalid("logprob owner"));
            }
            self.emit(
                StreamEvent::LogprobsSnapshot {
                    item,
                    part,
                    logprobs: super::super::text::read_logprobs(v)?,
                },
                out,
            )?;
        }
        self.emit(StreamEvent::ValueFinished { item, part }, out)?;
        if matches!(kind, PartKind::Arguments | PartKind::CustomInput) {
            self.emit(StreamEvent::PartFinished { item, part }, out)?;
        }
        Ok(())
    }
    fn part_done(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(
            o,
            &[
                "type",
                "output_index",
                "item_id",
                "content_index",
                "summary_index",
                "part",
            ],
        )?;
        let item = self.owner(o)?;
        let p = object(o.get("part").ok_or(CodecError::Invalid("part"))?)?;
        let kind = part_kind(p)?;
        let summary = string(o, "type")? == "response.reasoning_summary_part.done";
        if o.contains_key(if summary {
            "content_index"
        } else {
            "summary_index"
        }) {
            return Err(CodecError::Invalid("part index domain"));
        }
        if summary != (kind == PartKind::Summary) {
            return Err(CodecError::Invalid("part kind"));
        }
        let part = self.part_for(
            item,
            kind,
            index(
                o,
                if summary {
                    "summary_index"
                } else {
                    "content_index"
                },
            )?,
        )?;
        if part_text(p, kind)? != self.state()?.part(item, part)?.text {
            return Err(CodecError::Invalid("part snapshot"));
        }
        if kind == PartKind::Text {
            let t = super::super::text::read(p, false)?;
            self.emit(
                StreamEvent::TextMetadata {
                    item,
                    part,
                    annotations: t.annotations().to_vec(),
                    logprobs: t.logprobs().clone(),
                },
                out,
            )?;
        }
        self.emit(StreamEvent::PartFinished { item, part }, out)
    }
    fn item_done(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(o, &["type", "output_index", "item"])?;
        let item = *self
            .ids
            .get(index(o, "output_index")?)
            .ok_or(CodecError::Invalid("output index"))?;
        let v = o.get("item").ok_or(CodecError::Invalid("item"))?;
        let p = object(v)?;
        let status = if matches!(self.state()?.item(item)?.kind, ItemKind::CustomCall { .. }) {
            ItemLifecycle::Completed
        } else {
            match p.get("status").filter(|v| !v.is_null()) {
                Some(v) => lifecycle(v.as_str().ok_or(CodecError::Invalid("item status"))?)?,
                None => ItemLifecycle::Completed,
            }
        };
        // Reasoning-text may omit generic content-part.done; its value.done still closes the value.
        let parts = self.state()?.item(item)?.parts.clone();
        for part in parts {
            if part.kind == PartKind::ReasoningText && part.value_finished && !part.finished {
                self.emit(
                    StreamEvent::PartFinished {
                        item,
                        part: part.id,
                    },
                    out,
                )?;
            }
        }
        let token = if matches!(self.state()?.item(item)?.kind, ItemKind::Reasoning) {
            replay(p, self.origin.as_ref(), true)?
        } else {
            None
        };
        self.emit(
            StreamEvent::ItemFinished {
                item,
                status,
                replay: token,
            },
            out,
        )?;
        let expected = item_wire(self.state()?, item, &self.fidelity)?;
        let wrapper = json!({"id":"snapshot","object":"response","created_at":0,"model":"snapshot","status":"incomplete","output":[v]});
        let decoded = super::super::static_response::decode_responses(&wrapper)?;
        let normalized = super::super::responses::encode_items(
            decoded.semantic.items(),
            &decoded.fidelity,
            true,
        );
        if normalized.first() != Some(&expected) {
            return Err(CodecError::Invalid("item snapshot"));
        }
        Ok(())
    }
    fn terminal(
        &mut self,
        o: &Map<String, Value>,
        out: &mut Vec<StreamEvent>,
    ) -> Result<(), CodecError> {
        event_fields(o, &["type", "response"])?;
        let r = o.get("response").ok_or(CodecError::Invalid("response"))?;
        let p = object(r)?;
        let status = string(p, "status")?;
        if string(o, "type")? != format!("response.{status}") {
            return Err(CodecError::Invalid("terminal status"));
        }
        let terminal = match status {
            "completed" => StreamTerminal::Completed,
            "incomplete" => StreamTerminal::Incomplete,
            "failed" => StreamTerminal::Failed,
            "cancelled" => StreamTerminal::Cancelled,
            _ => return Err(CodecError::Invalid("terminal")),
        };
        self.observe_metadata(p)?;
        let decoded = super::super::static_response::decode_responses(r)?;
        sync_replays(
            self.state.as_ref().ok_or(CodecError::Invalid("state"))?,
            &mut self.fidelity,
            None,
        )?;
        let expected = super::super::responses::encode_items(
            &snapshot_items(self.state()?)?,
            &self.fidelity,
            true,
        );
        let actual = super::super::responses::encode_items(
            decoded.semantic.items(),
            &decoded.fidelity,
            true,
        );
        if actual != expected {
            return Err(CodecError::Invalid("terminal snapshot"));
        }
        if let Some(usage) = decoded.semantic.usage() {
            self.emit(StreamEvent::Usage(usage), out)?;
        }
        self.emit(
            StreamEvent::Terminal {
                terminal,
                details: decoded.semantic.details().clone(),
            },
            out,
        )
    }
}
