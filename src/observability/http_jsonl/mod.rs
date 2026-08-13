//! Bounded, UTC-rolling JSONL snapshot writer for authenticated downstream HTTP boundaries.
//!
//! Each content snapshot is serialized as one JSON line to a daily-rolled file. A dedicated
//! std thread owns the filesystem handle and drains a bounded channel. Short enqueue timeouts
//! and bounded drain windows ensure the writer never blocks or delays the HTTP request path.

mod record;
mod redaction;
mod writer;

pub(crate) use record::JsonlRecord;
pub use writer::HttpJsonlWriter;
