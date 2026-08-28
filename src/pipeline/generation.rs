//! Generation request semantics, fixed-interface preflight, and Route planning.
//!
//! This family is pure: it parses Chat Completions and Responses request facts, validates one fixed
//! Public Model interface, and prepares immutable Native or Bridged candidates without performing
//! transport, credential, response-body, or downstream commit I/O.

mod analysis;
pub(super) mod error;
mod instructions;
mod planning;
mod preflight;
mod response;
pub(super) mod types;

pub use analysis::analyze_request;
pub(crate) use instructions::normalize_probe_generation_request;
pub use planning::plan_request;
pub(crate) use response::{
    GenerationResponseFacts, GenerationResponseMode, classify_generation_response,
};
