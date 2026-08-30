//! Provider-neutral static Generation IR facade.
//!
//! Leaf modules own validated values, ordered requests/responses, semantic requirements, and
//! fidelity. Wire codecs, routing, Provider execution, transport, and observation remain outside
//! this module.

mod control;
mod event;
mod fidelity;
mod projection;
mod request;
mod requirements;
mod response;
mod semantic;
mod source;
mod tool;
mod value;

pub use control::*;
pub use event::*;
pub use fidelity::*;
pub use projection::*;
pub use request::*;
pub use requirements::*;
pub use response::*;
pub use semantic::*;
pub use source::*;
pub use tool::*;
pub use value::*;
