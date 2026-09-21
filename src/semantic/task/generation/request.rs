use std::collections::BTreeSet;
use crate::semantic::value::Text;
#[derive(Clone,Copy,Debug,Eq,Ord,PartialEq,PartialOrd)] pub struct ItemId(u64);
impl ItemId{pub const fn new(v:u64)->Self{Self(v)} pub const fn get(self)->u64{self.0}}
#[derive(Clone,Copy,Debug,Eq,Ord,PartialEq,PartialOrd)] pub struct PartId(u64);
impl PartId{pub const fn new(v:u64)->Self{Self(v)}}
#[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum InstructionAuthority{System,Developer}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct Instruction{pub authority:InstructionAuthority,pub text:Text}
#[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum MessageRole{User,Assistant}
#[derive(Clone,Debug,Eq,PartialEq)] pub enum ContentPart{Text(Text)}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct Part{pub id:PartId,pub content:ContentPart}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct Message{pub role:MessageRole,pub parts:Vec<Part>}
#[derive(Clone,Debug,Eq,PartialEq)] pub enum Item{Instruction(Instruction),Message(Message)}
#[derive(Clone,Debug,Eq,PartialEq,Default)] pub struct GenerationControls{pub max_output_tokens:Option<u64>,temperature_bits:Option<u64>}
impl GenerationControls{
 pub fn with_temperature(mut self,v:f64)->Result<Self,GenerationError>{if !v.is_finite(){return Err(GenerationError::NonFiniteTemperature)} self.temperature_bits=Some(v.to_bits());Ok(self)}
 pub fn temperature(&self)->Option<f64>{self.temperature_bits.map(f64::from_bits)}
}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct GenerationRequest{items:Vec<(ItemId,Item)>,controls:GenerationControls}
impl GenerationRequest{
 pub fn new(items:Vec<(ItemId,Item)>,controls:GenerationControls)->Result<Self,GenerationError>{
  if items.is_empty(){return Err(GenerationError::EmptyInput)}
  if items.iter().map(|x|x.0).collect::<BTreeSet<_>>().len()!=items.len(){return Err(GenerationError::DuplicateItemId)}
  for (_,item) in &items{if let Item::Message(m)=item{if m.parts.is_empty(){return Err(GenerationError::EmptyMessage)} if m.parts.iter().map(|p|p.id).collect::<BTreeSet<_>>().len()!=m.parts.len(){return Err(GenerationError::DuplicatePartId)}}}
  Ok(Self{items,controls})
 }
 pub fn items(&self)->&[(ItemId,Item)]{&self.items}
 pub const fn controls(&self)->&GenerationControls{&self.controls}
 pub fn retain_items(mut self,mut keep:impl FnMut(ItemId,&Item)->bool)->Result<Self,GenerationError>{self.items.retain(|(id,item)|keep(*id,item));if self.items.is_empty(){return Err(GenerationError::EmptyInput)}Ok(self)}
}
#[derive(Clone,Debug,Eq,thiserror::Error,PartialEq)] pub enum GenerationError{
 #[error("generation input must not be empty")] EmptyInput,
 #[error("generation item identity must be unique")] DuplicateItemId,
 #[error("message content must not be empty")] EmptyMessage,
 #[error("message part identity must be unique")] DuplicatePartId,
 #[error("temperature must be finite")] NonFiniteTemperature,
}
