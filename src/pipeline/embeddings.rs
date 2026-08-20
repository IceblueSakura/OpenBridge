//! Embeddings Create request semantics, fixed-interface preflight, and Route planning.
//!
//! This family is pure: it parses request facts and binds an immutable execution interface without
//! performing transport, credential, response-body, or downstream commit I/O.

mod analysis;
mod planning;
mod preflight;

pub use analysis::analyze_embedding_request;
pub use planning::plan_embedding_request;
