//! Generation semantic IR.
mod request;
mod requirements;
pub use request::{ContentPart,GenerationControls,GenerationRequest,Instruction,InstructionAuthority,Item,ItemId,Message,MessageRole,Part,PartId};
pub use requirements::GenerationRequirements;
