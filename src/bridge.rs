//! Protocol Bridge facade backed exclusively by canonical Generation Static/Event IR.
//!
//! The Bridge owns pure Chat ↔ Responses lowering and per-request Event state. Routing, Provider
//! selection, transport I/O, precommit, liveness, cancellation, and downstream commit remain with
//! their existing owners.

mod event_codec;
mod static_codec;

pub use event_codec::{StaticEventBridge, StaticEventCodecError};
pub use static_codec::{
    BridgeError, BridgeLimits, BridgePlan, BridgeStreamRenderer, ProviderToolTarget,
    StaticBridgePlan, StaticCodecError, StaticCodecLimits, StaticRenderedResponse,
};
