//! OpenAI-compatible protocol profiles.
pub mod chat;
pub mod responses;
#[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum OpenAiProfile{ChatCompletions,Responses}
