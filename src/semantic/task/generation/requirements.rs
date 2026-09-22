use super::{ContentPart,GenerationRequest,Item,OutputConstraint,ResourceKind,ToolDefinition};
#[derive(Clone,Debug,Default,Eq,PartialEq)]
pub struct GenerationRequirements{
 pub instruction_count:usize,pub message_count:usize,pub text_part_count:usize,
 pub resource_count:usize,pub image_inputs:usize,pub audio_inputs:usize,pub file_inputs:usize,
 pub tool_count:usize,pub tool_history:bool,pub structured_output:bool,pub reasoning:bool,
 pub max_output_tokens:Option<u64>,pub temperature:bool
}
impl GenerationRequirements{
 pub fn derive(r:&GenerationRequest)->Self{
  let mut x=Self{max_output_tokens:r.controls().max_output_tokens,temperature:r.controls().temperature().is_some(),tool_count:r.tools().len(),structured_output:!matches!(r.output(),OutputConstraint::Text),reasoning:r.reasoning().effort.is_some()||r.reasoning().summary.is_some(),..Self::default()};
  for(_,i)in r.items(){match i{
   Item::Instruction(_)=>x.instruction_count+=1,
   Item::Message(m)=>{x.message_count+=1;for p in &m.parts{match &p.content{ContentPart::Text(_)=>x.text_part_count+=1,ContentPart::Resource(resource)=>{x.resource_count+=1;match resource.kind{ResourceKind::Image=>x.image_inputs+=1,ResourceKind::Audio=>x.audio_inputs+=1,ResourceKind::File=>x.file_inputs+=1}}}}},
   Item::ToolCall(_)|Item::ToolResult(_)=>x.tool_history=true,
  }}
  for t in r.tools(){match t{ToolDefinition::Function(_)=>{}}}
  x
 }
}
