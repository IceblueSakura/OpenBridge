use serde_json::{Value,json};
use crate::{protocol::fidelity::FidelityRecords,semantic::{task::generation::{ContentPart,GenerationControls,GenerationRequest,Instruction,InstructionAuthority,Item,ItemId,Message,MessageRole,Part,PartId},value::Text}};
#[derive(Clone,Debug,PartialEq)] pub struct DecodedResponses{pub semantic:GenerationRequest,pub fidelity:FidelityRecords}
pub fn decode_generation(v:&Value)->Result<DecodedResponses,ResponsesCodecError>{
 let o=v.as_object().ok_or(ResponsesCodecError::ExpectedObject)?;let input=o.get("input").and_then(Value::as_array).ok_or(ResponsesCodecError::MissingInput)?;
 let(mut items,mut iid,mut pid)=(Vec::new(),1u64,1u64);
 for x in input{let x=x.as_object().ok_or(ResponsesCodecError::InvalidItem)?;let role=x.get("role").and_then(Value::as_str).ok_or(ResponsesCodecError::InvalidItem)?;let id=ItemId::new(iid);iid+=1;
  if role=="system"||role=="developer"{let s=x.get("content").and_then(Value::as_str).ok_or(ResponsesCodecError::InvalidItem)?;let t=Text::new(s,"instruction",1<<20).map_err(|_|ResponsesCodecError::InvalidItem)?;items.push((id,Item::Instruction(Instruction{authority:if role=="system"{InstructionAuthority::System}else{InstructionAuthority::Developer},text:t})));continue}
  let parts=x.get("content").and_then(Value::as_array).ok_or(ResponsesCodecError::InvalidItem)?;let mut out=Vec::new();for p in parts{let p=p.as_object().ok_or(ResponsesCodecError::InvalidItem)?;let typ=p.get("type").and_then(Value::as_str).ok_or(ResponsesCodecError::InvalidItem)?;if typ!="input_text"&&typ!="output_text"{return Err(ResponsesCodecError::UnsupportedPart)}let s=p.get("text").and_then(Value::as_str).ok_or(ResponsesCodecError::InvalidItem)?;out.push(Part{id:PartId::new(pid),content:ContentPart::Text(Text::new(s,"text",1<<20).map_err(|_|ResponsesCodecError::InvalidItem)?)});pid+=1}
  items.push((id,Item::Message(Message{role:if role=="user"{MessageRole::User}else if role=="assistant"{MessageRole::Assistant}else{return Err(ResponsesCodecError::UnsupportedRole)},parts:out})));
 }
 let mut c=GenerationControls{max_output_tokens:o.get("max_output_tokens").and_then(Value::as_u64),..Default::default()};if let Some(t)=o.get("temperature").and_then(Value::as_f64){c=c.with_temperature(t).map_err(|_|ResponsesCodecError::InvalidControl)?}
 Ok(DecodedResponses{semantic:GenerationRequest::new(items,c).map_err(|_|ResponsesCodecError::InvalidSemantic)?,fidelity:FidelityRecords::default()})
}
pub fn encode_generation(r:&GenerationRequest)->Value{let mut input=Vec::new();for(_,i)in r.items(){match i{
 Item::Instruction(x)=>input.push(json!({"role":match x.authority{InstructionAuthority::System=>"system",InstructionAuthority::Developer=>"developer"},"content":x.text.as_str()})),
 Item::Message(m)=>{let c=m.parts.iter().filter_map(|p|match &p.content{ContentPart::Text(t)=>Some(json!({"type":if m.role==MessageRole::User{"input_text"}else{"output_text"},"text":t.as_str()})),ContentPart::Resource(_)=>None}).collect::<Vec<_>>();input.push(json!({"role":match m.role{MessageRole::User=>"user",MessageRole::Assistant=>"assistant"},"content":c}))},
 Item::ToolCall(_)|Item::ToolResult(_)=>{}
 }}let mut v=json!({"input":input});let o=v.as_object_mut().unwrap();if let Some(x)=r.controls().max_output_tokens{o.insert("max_output_tokens".into(),json!(x));}if let Some(x)=r.controls().temperature(){o.insert("temperature".into(),json!(x));}v}
#[derive(Clone,Debug,Eq,thiserror::Error,PartialEq)] pub enum ResponsesCodecError{#[error("expected object")]ExpectedObject,#[error("input required")]MissingInput,#[error("invalid item")]InvalidItem,#[error("unsupported role")]UnsupportedRole,#[error("unsupported part")]UnsupportedPart,#[error("invalid control")]InvalidControl,#[error("invalid semantics")]InvalidSemantic}
