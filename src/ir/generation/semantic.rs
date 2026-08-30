//! Owned semantic leaf values shared by static Generation request and response IR.
//!
//! Leaf values validate only local shape and caller-supplied bounds. They perform no I/O and do
//! not resolve Providers, Routes, credentials, or transports.

mod extension;
mod leaf;
mod reasoning;
mod resource;
mod state;

pub use extension::*;
pub use leaf::*;
pub use reasoning::*;
pub use resource::*;
pub use state::*;
