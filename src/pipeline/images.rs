//! Images Generations request semantics, fixed-interface preflight, Route planning, and response policy.
//!
//! This family is pure: it parses request facts and binds an immutable execution interface without
//! performing transport, credential, response-body, or downstream commit I/O.

mod analysis;
mod planning;
mod preflight;
mod response;

pub use analysis::analyze_images_request;
pub use planning::plan_images_request;
pub(crate) use response::{
    ImagesResponseError, validate_images_response_body, validate_images_response_headers,
};
