//! OpenBridge v2 semantic core, protocol codecs, target lowering, trusted
//! provider/topology bindings and a minimal execution chain.
//!
//! The gateway module connects these pure layers to bounded HTTP I/O. The
//! optional-use binary loads explicit environment credentials and listens only
//! on loopback; library construction never reads private configuration.

pub mod adapter;
pub mod execution;
pub mod gateway;
pub mod lowering;
pub mod protocol;
pub mod provider;
pub mod semantic;
pub mod topology;
pub mod transport;
