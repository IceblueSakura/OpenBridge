//! OpenBridge v2 semantic core, protocol codecs, target lowering, trusted
//! provider/topology bindings and a minimal execution chain.
//!
//! This crate does not provide a gateway server, credential material or network
//! I/O: transport is supplied by callers. HTTP integration is exercised only by
//! synthetic loopback acceptance tests and the explicitly authorized probe entry.

pub mod execution;
pub mod lowering;
pub mod protocol;
pub mod provider;
pub mod semantic;
pub mod topology;
pub mod transport;
