//! 仅用于 bootstrap TOML 反序列化的私有文档模型。
//!
//! Provider、Model、Upstream Target、Upstream API、Route 与 Public Model 均由 Rust 代码注册，不属于运行时配置。

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawBootstrap {
    pub(super) schema_version: u32,
    pub(super) listen: String,
    pub(super) users_file: PathBuf,
    pub(super) max_request_body_bytes: usize,
    pub(super) max_sse_event_bytes: usize,
    pub(super) upstream_connect_timeout_ms: u64,
    pub(super) upstream_pool_idle_timeout_ms: u64,
    pub(super) upstream_pool_max_idle_per_host: usize,
}
