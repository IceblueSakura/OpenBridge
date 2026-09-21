use std::collections::BTreeMap;
use serde_json::Value;
use crate::semantic::task::generation::ItemId;
#[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum Portability{SemanticPortable,SameProfileOnly,SameProviderOnly,NonReplayable}
#[derive(Clone,Debug,Default,PartialEq)] pub struct FidelityRecords{item_unknown:BTreeMap<ItemId,BTreeMap<String,Value>>}
impl FidelityRecords{pub fn insert_item_unknown(&mut self,id:ItemId,key:String,value:Value){self.item_unknown.entry(id).or_default().insert(key,value)} pub fn item_unknown(&self,id:ItemId)->Option<&BTreeMap<String,Value>>{self.item_unknown.get(&id)}}
