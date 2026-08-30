//! Provider-neutral intermediate representations.
//!
//! This module owns semantic values and pure validation/projection only. Wire codecs, routing,
//! Provider execution, transport, and observation remain in their existing owner modules.

pub mod generation;
