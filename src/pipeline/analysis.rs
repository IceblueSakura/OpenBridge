//! Registry-independent request analysis grouped by operation family.
//!
//! Generation and Embeddings requests have separate wire contracts and produce separate requirement
//! types. This facade keeps the public pipeline API stable without coupling either analyzer to
//! registry lookup, Route selection, or request rewriting.

mod generation;

pub use generation::analyze_request;
