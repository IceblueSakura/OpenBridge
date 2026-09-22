//! Event rendering uses only validated semantic state and owner-bound wire identities.
use super::*;
use serde_json::json;

pub struct EventEncoder {
    pub(super) profile: Profile,
    pub(super) metadata: ResponseMetadata,
    pub(super) state: Option<StreamState>,
    pub(super) fidelity: FidelityRecords,
    pub(super) contract: GenerationRepresentationContract,
    poisoned: bool,
    sequence: u64,
}
impl EventEncoder {
    pub fn new(profile: Profile, metadata: ResponseMetadata) -> Result<Self, CodecError> {
        if metadata.id.is_empty()
            || metadata.model.is_empty()
            || metadata.id.len() > 256
            || metadata.model.len() > 256
        {
            return Err(CodecError::Invalid("metadata"));
        }
        Ok(Self {
            profile,
            metadata,
            state: Some(StreamState::new()),
            fidelity: FidelityRecords::default(),
            contract: GenerationRepresentationContract::full(),
            poisoned: false,
            sequence: 0,
        })
    }
    pub fn with_contract(mut self, contract: GenerationRepresentationContract) -> Self {
        self.contract = contract;
        self
    }
    pub fn encode(
        &mut self,
        event: &StreamEvent,
        source: &FidelityRecords,
    ) -> Result<Vec<Value>, CodecError> {
        if self.poisoned {
            return Err(CodecError::Invalid("rejected stream"));
        }
        let result = (|| {
            crate::lowering::events::check_event(
                self.state()?,
                event,
                self.profile,
                &self.contract,
            )
            .map_err(|_| CodecError::Unsupported("event target representation".into()))?;
            if let StreamEvent::ItemStarted { item, .. } = event {
                let id = source
                    .response_item_id(*item)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("item_{}", item.get()));
                self.fidelity.record_response_item_id(*item, &id)?;
            }
            self.state = Some(reduce(
                self.state.take().ok_or(CodecError::Invalid("state"))?,
                event.clone(),
            )?);
            // Replay is event authority, never restored from the source sidecar.
            if let StreamEvent::ItemFinished {
                item,
                replay: Some(replay),
                ..
            } = event
                && let Item::Reasoning(owner) = self.state()?.item(*item)?.snapshot()?
            {
                self.fidelity.record_replay(*item, replay.clone(), &owner)?;
            }
            let mut values = if self.profile == Profile::Responses {
                self.responses(event)?
            } else {
                self.chat(event)?
            };
            for v in &mut values {
                if self.profile == Profile::Responses {
                    v["sequence_number"] = json!(self.sequence);
                    self.sequence += 1;
                }
                bounded(v)?;
            }
            Ok(values)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub fn finish(&self) -> Result<(), CodecError> {
        if self.poisoned {
            return Err(CodecError::Invalid("rejected stream"));
        }
        end_of_stream(self.state()?)?;
        Ok(())
    }
    pub(super) fn state(&self) -> Result<&StreamState, CodecError> {
        self.state.as_ref().ok_or(CodecError::Invalid("state"))
    }
    fn index(&self, id: ItemId) -> Result<usize, CodecError> {
        self.state()?
            .items()
            .iter()
            .position(|i| i.id == id)
            .ok_or(CodecError::Invalid("item identity"))
    }
    fn part_index(&self, item: ItemId, part: PartId) -> Result<usize, CodecError> {
        let kind = self.state()?.part(item, part)?.kind;
        self.state()?
            .item(item)?
            .parts
            .iter()
            .filter(|p| (p.kind == PartKind::Summary) == (kind == PartKind::Summary))
            .position(|p| p.id == part)
            .ok_or(CodecError::Invalid("part identity"))
    }
    fn coordinates(&self, item: ItemId, part: PartId) -> Result<Value, CodecError> {
        let kind = self.state()?.part(item, part)?.kind;
        let mut v = json!({"output_index":self.index(item)?,"item_id":self.fidelity.response_item_id(item).ok_or(CodecError::Invalid("wire identity"))?});
        if kind != PartKind::Arguments {
            v[if kind == PartKind::Summary {
                "summary_index"
            } else {
                "content_index"
            }] = json!(self.part_index(item, part)?);
        }
        Ok(v)
    }
    pub(super) fn envelope(&self, status: &str, output: Vec<Value>) -> Value {
        json!({"id":self.metadata.id,"object":"response","created_at":self.metadata.created,"model":self.metadata.model,"status":status,"output":output})
    }
    fn responses(&self, event: &StreamEvent) -> Result<Vec<Value>, CodecError> {
        let result = match event {
            StreamEvent::Started => vec![
                json!({"type":"response.created","response":self.envelope("in_progress",vec![])}),
                json!({"type":"response.in_progress","response":self.envelope("in_progress",vec![])}),
            ],
            StreamEvent::ItemStarted { item, kind, replay } => {
                let id = self
                    .fidelity
                    .response_item_id(*item)
                    .ok_or(CodecError::Invalid("wire identity"))?;
                let mut v = match kind {
                    ItemKind::Message => {
                        json!({"id":id,"type":"message","role":"assistant","content":[],"status":"in_progress"})
                    }
                    ItemKind::Reasoning => {
                        json!({"id":id,"type":"reasoning","summary":[],"status":"in_progress"})
                    }
                    ItemKind::ToolCall { call_id, name, .. } => {
                        json!({"id":id,"type":"function_call","call_id":call_id.as_str(),"name":name.as_str(),"arguments":"","status":"in_progress"})
                    }
                };
                if let Some(r) = replay {
                    v["encrypted_content"] = json!(r.value.as_str());
                }
                vec![
                    json!({"type":"response.output_item.added","output_index":self.index(*item)?,"item":v}),
                ]
            }
            StreamEvent::PartStarted { item, part, kind } => {
                if *kind == PartKind::Arguments {
                    vec![]
                } else {
                    let mut v = self.coordinates(*item, *part)?;
                    v["type"] = json!(if *kind == PartKind::Summary {
                        "response.reasoning_summary_part.added"
                    } else {
                        "response.content_part.added"
                    });
                    v["part"] = part_wire(*kind, "");
                    vec![v]
                }
            }
            StreamEvent::Delta {
                item,
                part,
                fragment,
            } => {
                let kind = self.state()?.part(*item, *part)?.kind;
                let mut v = self.coordinates(*item, *part)?;
                v["type"] = json!(format!("response.{}.delta", event_stem(kind)));
                v["delta"] = json!(fragment);
                vec![v]
            }
            StreamEvent::ValueFinished { item, part } => {
                let p = self.state()?.part(*item, *part)?;
                let mut v = self.coordinates(*item, *part)?;
                v["type"] = json!(format!("response.{}.done", event_stem(p.kind)));
                v[match p.kind {
                    PartKind::Arguments => "arguments",
                    PartKind::Refusal => "refusal",
                    _ => "text",
                }] = json!(p.text);
                vec![v]
            }
            StreamEvent::PartFinished { item, part } => {
                let p = self.state()?.part(*item, *part)?;
                if p.kind == PartKind::Arguments {
                    vec![]
                } else {
                    let mut v = self.coordinates(*item, *part)?;
                    v["type"] = json!(if p.kind == PartKind::Summary {
                        "response.reasoning_summary_part.done"
                    } else {
                        "response.content_part.done"
                    });
                    v["part"] = part_wire(p.kind, &p.text);
                    vec![v]
                }
            }
            StreamEvent::ItemFinished { item, .. } => vec![
                json!({"type":"response.output_item.done","output_index":self.index(*item)?,"item":item_wire(self.state()?,*item,&self.fidelity)?}),
            ],
            StreamEvent::Usage(_) => vec![],
            StreamEvent::Terminal {
                terminal: StreamTerminal::Error,
                details,
            } => {
                let mut v = super::super::terminal::encode_error(details.error.as_ref());
                if !v.is_object() {
                    return Err(CodecError::Invalid("error details"));
                }
                v["type"] = json!("error");
                vec![v]
            }
            StreamEvent::Terminal { terminal, .. } => {
                let response = materialize(self.state()?)?;
                let target = crate::lowering::generation::lower_response(
                    &response,
                    &self.fidelity,
                    &self.metadata,
                    Profile::Responses,
                    self.contract.clone(),
                )
                .map_err(|_| CodecError::Unsupported("terminal representation".into()))?;
                let v = super::super::responses::encode_response(&target)?;
                let kind = match terminal {
                    StreamTerminal::Completed => "response.completed",
                    StreamTerminal::Incomplete => "response.incomplete",
                    StreamTerminal::Failed => "response.failed",
                    StreamTerminal::Cancelled => "response.cancelled",
                    StreamTerminal::Error => unreachable!(),
                };
                vec![json!({"type":kind,"response":v})]
            }
        };
        Ok(result)
    }
}
fn event_stem(kind: PartKind) -> &'static str {
    match kind {
        PartKind::Text => "output_text",
        PartKind::Refusal => "refusal",
        PartKind::Summary => "reasoning_summary_text",
        PartKind::ReasoningText => "reasoning_text",
        PartKind::Arguments => "function_call_arguments",
    }
}
