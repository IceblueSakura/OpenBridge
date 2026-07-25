//! 仅用于 TOML 反序列化的私有文档模型。
//!
//! 它与编译后的 `RegistrySnapshot` 故意分离：文档可随 schema 版本演进，而请求热路径
//! 只读取已经验证、解析好的运行时值。

use serde::Deserialize;

use crate::core::{CapabilitySet, ProtocolCapabilities, ResponsesCapabilities};

use super::ModelLimits;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawBootstrap {
    pub(super) schema_version: u32,
    pub(super) listen: String,
    pub(super) allowed_origins: Vec<String>,
    pub(super) max_request_body_bytes: usize,
    pub(super) max_sse_event_bytes: usize,
    pub(super) upstream_connect_timeout_ms: u64,
    pub(super) upstream_pool_idle_timeout_ms: u64,
    pub(super) upstream_pool_max_idle_per_host: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRoutes {
    pub(super) schema_version: u32,
    pub(super) config_version: String,
    pub(super) providers: Vec<RawProvider>,
    pub(super) deployments: Vec<RawDeployment>,
    pub(super) aliases: Vec<RawAlias>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawProvider {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) credential: RawCredential,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCredential {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) secret_ref: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDeployment {
    pub(super) id: String,
    pub(super) provider: String,
    pub(super) upstream_model: String,
    pub(super) endpoint_profile: String,
    pub(super) base_url: String,
    pub(super) request_timeout_ms: u64,
    pub(super) capabilities: RawCapabilitySet,
    #[serde(default)]
    pub(super) model_limits: RawModelLimits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAlias {
    pub(super) name: String,
    pub(super) candidates: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCapabilitySet {
    chat_completions: RawProtocolCapabilities,
    responses: RawResponsesCapabilities,
}

impl From<RawCapabilitySet> for CapabilitySet {
    fn from(raw: RawCapabilitySet) -> Self {
        Self {
            chat_completions: raw.chat_completions.into(),
            responses: raw.responses.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProtocolCapabilities {
    enabled: bool,
    streaming: bool,
    function_calling: bool,
    parallel_tool_calls: bool,
    image_input: bool,
    structured_outputs: bool,
    store: bool,
}

impl From<RawProtocolCapabilities> for ProtocolCapabilities {
    fn from(raw: RawProtocolCapabilities) -> Self {
        Self {
            enabled: raw.enabled,
            streaming: raw.streaming,
            function_calling: raw.function_calling,
            parallel_tool_calls: raw.parallel_tool_calls,
            image_input: raw.image_input,
            structured_outputs: raw.structured_outputs,
            store: raw.store,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResponsesCapabilities {
    enabled: bool,
    streaming: bool,
    function_calling: bool,
    parallel_tool_calls: bool,
    image_input: bool,
    structured_outputs: bool,
    store: bool,
    previous_response_id: bool,
    background: bool,
}

impl From<RawResponsesCapabilities> for ResponsesCapabilities {
    fn from(raw: RawResponsesCapabilities) -> Self {
        Self {
            enabled: raw.enabled,
            streaming: raw.streaming,
            function_calling: raw.function_calling,
            parallel_tool_calls: raw.parallel_tool_calls,
            image_input: raw.image_input,
            structured_outputs: raw.structured_outputs,
            store: raw.store,
            previous_response_id: raw.previous_response_id,
            background: raw.background,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawModelLimits {
    context_window_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
}

impl From<RawModelLimits> for ModelLimits {
    fn from(raw: RawModelLimits) -> Self {
        Self::new(raw.context_window_tokens, raw.max_output_tokens)
    }
}
