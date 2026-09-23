//! OpenBridge v2 semantic core, protocol codecs and target lowering.
//!
//! This crate does not provide a gateway server, provider execution, credentials or routing.
//! HTTP integration is exercised only by synthetic loopback acceptance tests.

pub mod lowering;
pub mod protocol;
pub mod semantic;
pub mod transport;
