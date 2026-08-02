//! 管理员显式执行的上游 capability probe 门面。
//!
//! probe 复用 Upstream Target 的受信 endpoint、credential 和编译期 adapter，但不走下游
//! HTTP API，也不修改代码注册表。`session` 负责受信执行，`payload` 负责固定 wire 请求与
//! 响应形状；公开报告只作为服务所有者更新 capability 配置时的证据。

use http::StatusCode;
use serde::Serialize;
use thiserror::Error;

mod payload;
mod session;

pub use session::probe_upstream_target;

/// 明确选择要执行的 probe。CLI 不传任何选择时使用 `all()`；库调用方可仅执行无费用的
/// `list_models`，或只验证特定协议。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProbeOptions {
    /// 是否执行 `/v1/models` probe。
    pub list_models: bool,
    /// 是否执行 Chat Completions 文本请求 probe。
    pub chat: bool,
    /// 是否执行 Responses 文本请求 probe。
    pub responses: bool,
    /// 是否执行 function call 及结果回放 probe。
    pub function_calling: bool,
}

impl ProbeOptions {
    /// 选择全部已实现的 probe。
    pub const fn all() -> Self {
        Self {
            list_models: true,
            chat: true,
            responses: true,
            function_calling: true,
        }
    }

    /// 判断是否没有选择任何 probe。
    pub const fn is_empty(self) -> bool {
        !self.list_models && !self.chat && !self.responses && !self.function_calling
    }
}

/// probe 对某个能力的保守结论。
///
/// `unsupported` 只用于端点明确不存在（404/405/501）。认证、限流、网络故障及请求
/// 形状被拒绝均保留为 `unknown`，避免一次临时故障错误关闭一条路由能力。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    /// 请求符合该 probe 预期的协议形状。
    Supported,
    /// endpoint 明确返回不支持该操作的 status。
    Unsupported,
    /// 请求失败或响应形状不足以作出结论。
    Unknown,
}

/// 单项 probe 的状态和可选 HTTP status。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProbeResult {
    /// 本次 probe 的保守结论。
    pub state: SupportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 上游返回的 HTTP status；尚未收到响应时为空。
    pub http_status: Option<u16>,
}

impl ProbeResult {
    const fn supported(status: StatusCode) -> Self {
        Self {
            state: SupportStatus::Supported,
            http_status: Some(status.as_u16()),
        }
    }

    const fn from_http_status(status: StatusCode) -> Self {
        Self {
            state: if matches!(
                status,
                StatusCode::NOT_FOUND
                    | StatusCode::METHOD_NOT_ALLOWED
                    | StatusCode::NOT_IMPLEMENTED
            ) {
                SupportStatus::Unsupported
            } else {
                SupportStatus::Unknown
            },
            http_status: Some(status.as_u16()),
        }
    }

    const fn unknown(status: Option<StatusCode>) -> Self {
        Self {
            state: SupportStatus::Unknown,
            http_status: match status {
                Some(status) => Some(status.as_u16()),
                None => None,
            },
        }
    }
}

/// `/v1/models` probe 的模型列表观察结果。
#[derive(Debug, Serialize)]
pub struct ModelListProbeResult {
    /// `/v1/models` 请求本身的结论。
    pub outcome: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 配置的 upstream model 是否出现在返回列表中。
    pub configured_model_listed: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// 从响应中提取的 model id，可能为空或不完整。
    pub model_ids: Vec<String>,
}

/// function calling probe 及其 tool-result replay 的观察结果。
#[derive(Debug, Serialize)]
pub struct ToolCallProbeResult {
    /// 初始 function call 请求结论。
    pub initial_call: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 将 tool result 回放后的请求结论。
    pub result_replay: Option<ProbeResult>,
}

/// 单个 Upstream Target 的 probe 报告。它不包含 credential、请求正文或上游响应正文。
#[derive(Debug, Serialize)]
pub struct TargetProbeReport {
    /// 被 probe 的内部 target id。
    pub upstream_target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `/v1/models` 的观察结果。
    pub list_models: Option<ModelListProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Chat Completions 文本 probe 的观察结果。
    pub chat: Option<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Responses 文本 probe 的观察结果。
    pub responses: Option<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Chat Completions function calling probe 的观察结果。
    pub chat_function_calling: Option<ToolCallProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Responses function calling probe 的观察结果。
    pub responses_function_calling: Option<ToolCallProbeResult>,
}

#[derive(Debug, Error)]
/// probe 准备阶段失败。
pub enum ProbeError {
    /// 请求的 Upstream Target 未注册。
    #[error("configured upstream target '{upstream_target}' does not exist")]
    UnknownUpstreamTarget {
        /// 未找到的内部 target id。
        upstream_target: String,
    },
    /// 受信 credential source 无法提供所需 secret。
    #[error("upstream credentials are unavailable for probe")]
    CredentialUnavailable,
    /// adapter 无法为 probe 构造认证 header。
    #[error("provider authentication could not be prepared for probe")]
    AuthenticationPreparation,
}

#[cfg(test)]
mod tests;
