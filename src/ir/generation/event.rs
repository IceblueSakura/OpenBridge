//! Canonical Generation Event IR, pure reducer, and Static materializer.
//!
//! Events model one Provider turn independently from HTTP/SSE framing, retry, downstream commit,
//! tool execution, clocks, tasks, and observation. The reducer owns lifecycle, identity, usage, EOF,
//! and bounded accumulation; the materializer produces the same validated Static IR used by
//! non-stream codecs.

mod algebra;
mod materialize;
mod reducer;

pub use algebra::*;
pub use materialize::*;
pub use reducer::*;
