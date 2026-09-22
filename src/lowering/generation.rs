use crate::semantic::task::generation::{GenerationRequest,GenerationRequirements};
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct GenerationRepresentationContract{
 pub instructions:bool,pub temperature:bool,pub max_output_tokens:bool,pub tools:bool,
 pub structured_output:bool,pub reasoning:bool,pub image_input:bool,pub audio_input:bool,pub file_input:bool,
}
impl GenerationRepresentationContract{pub const fn full()->Self{Self{instructions:true,temperature:true,max_output_tokens:true,tools:true,structured_output:true,reasoning:true,image_input:true,audio_input:true,file_input:true}}}
pub fn check(r:&GenerationRequest,c:GenerationRepresentationContract)->Result<GenerationRequirements,RepresentationError>{
 let q=GenerationRequirements::derive(r);
 if q.instruction_count>0&&!c.instructions{return Err(RepresentationError::Instructions)}
 if q.temperature&&!c.temperature{return Err(RepresentationError::Temperature)}
 if q.max_output_tokens.is_some()&&!c.max_output_tokens{return Err(RepresentationError::MaxOutputTokens)}
 if (q.tool_count>0||q.tool_history)&&!c.tools{return Err(RepresentationError::Tools)}
 if q.structured_output&&!c.structured_output{return Err(RepresentationError::StructuredOutput)}
 if q.reasoning&&!c.reasoning{return Err(RepresentationError::Reasoning)}
 if q.image_inputs>0&&!c.image_input{return Err(RepresentationError::ImageInput)}
 if q.audio_inputs>0&&!c.audio_input{return Err(RepresentationError::AudioInput)}
 if q.file_inputs>0&&!c.file_input{return Err(RepresentationError::FileInput)}
 Ok(q)
}
#[derive(Clone,Copy,Debug,Eq,thiserror::Error,PartialEq)] pub enum RepresentationError{
 #[error("target cannot represent instructions")]Instructions,#[error("target cannot represent temperature")]Temperature,
 #[error("target cannot represent maximum output tokens")]MaxOutputTokens,#[error("target cannot represent tools")]Tools,
 #[error("target cannot represent structured output")]StructuredOutput,#[error("target cannot represent reasoning")]Reasoning,
 #[error("target cannot represent image input")]ImageInput,#[error("target cannot represent audio input")]AudioInput,
 #[error("target cannot represent file input")]FileInput,
}
