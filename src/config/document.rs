//! Private document model used only to deserialize bootstrap TOML.
//!
//! Provider, Model, Upstream Target, Upstream API, Route, and Public Model entries are registered
//! in Rust code and are not runtime configuration.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawBootstrap {
    pub(super) schema_version: u32,
    pub(super) listen: String,
    pub(super) users_file: PathBuf,
    pub(super) upstream_credentials_file: PathBuf,
    pub(super) chatgpt_instructions: Option<String>,
    pub(super) max_request_body_bytes: usize,
    pub(super) max_json_response_body_bytes: usize,
    pub(super) max_replay_body_bytes: usize,
    pub(super) max_sse_event_bytes: usize,
    pub(super) upstream_connect_timeout_ms: u64,
    pub(super) upstream_pool_idle_timeout_ms: u64,
    pub(super) upstream_pool_max_idle_per_host: usize,
    pub(super) telemetry: Option<RawTelemetry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTelemetry {
    pub(super) traces: Option<RawOtlpHttpExport>,
    pub(super) metrics: Option<RawOtlpHttpExport>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawOtlpHttpExport {
    pub(super) otlp_http_endpoint: String,
}
