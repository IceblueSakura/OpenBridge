//! Wire protocol boundary for semantic-v2.
pub mod adaptation;
pub mod cache;
mod decoded;
pub use decoded::{CodecError, DecodedRequest, DecodedResponse, ResponseMetadata};
pub mod extensions;
pub mod fidelity;
pub mod file_constraints;
pub mod image_constraints;
pub mod openai;
pub mod openrouter_images;
pub mod openrouter_speech;
