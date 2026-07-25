//! 仅用于 TOML 反序列化的私有文档模型。
//!
//! 它与编译后的 `RegistrySnapshot` 故意分离：文档可随 schema 版本演进，而请求热路径
//! 只读取已经验证、解析好的运行时值。

use serde::Deserialize;

use crate::core::{CapabilitySet, ProtocolCapabilities, ResponsesCapabilities};

use super::{ModelContextLength, ReasoningSupport};

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
    pub(super) models: Vec<RawModel>,
    pub(super) providers: Vec<RawProvider>,
    pub(super) deployments: Vec<RawDeployment>,
    pub(super) aliases: Vec<RawAlias>,
}

/// 与 OpenRouter model catalog 同一职责层级的 owner-maintained 模型目录项。
///
/// `context_length.input`、`context_length.output` 和 `reasoning` 可以未知；未知不应
/// 被路由逻辑外推为支持。provider 原生 model id 仍属于 deployment。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawModel {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) context_length: RawModelContextLength,
    #[serde(default)]
    pub(super) supported_parameters: Vec<String>,
    #[serde(default)]
    pub(super) reasoning: RawReasoningSupport,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawModelContextLength {
    pub(super) input: Option<u32>,
    pub(super) output: Option<u32>,
}

impl From<RawModelContextLength> for ModelContextLength {
    fn from(raw: RawModelContextLength) -> Self {
        Self::new(raw.input, raw.output)
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RawReasoningSupport {
    #[default]
    Unknown,
    Supported,
    Unsupported,
}

impl From<RawReasoningSupport> for ReasoningSupport {
    fn from(raw: RawReasoningSupport) -> Self {
        match raw {
            RawReasoningSupport::Unknown => ReasoningSupport::Unknown,
            RawReasoningSupport::Supported => ReasoningSupport::Supported,
            RawReasoningSupport::Unsupported => ReasoningSupport::Unsupported,
        }
    }
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
    pub(super) model: String,
    pub(super) upstream_model: String,
    pub(super) endpoint_profile: String,
    pub(super) base_url: String,
    pub(super) request_timeout_ms: u64,
    pub(super) capabilities: RawCapabilitySet,
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
