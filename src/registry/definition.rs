//! 注册表编译前的静态定义。

use std::time::Duration;

use crate::{
    core::{
        ApiCapabilities, ApiProtocol, ChatCompletionsCapabilities, GenerationCapabilities,
        ResponsesCapabilities,
    },
    provider::{CredentialKind, ProviderKind},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// 模型 reasoning 能力的证据状态。
pub enum ReasoningSupport {
    #[default]
    /// 配置没有足够证据判断是否支持 reasoning。
    Unknown,
    /// 模型明确支持 reasoning。
    Supported,
    /// 模型明确不支持 reasoning。
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// 模型支持的 reasoning 强度。
pub enum ReasoningLevel {
    /// 显式禁用 reasoning。
    None,
    /// 最低 reasoning 强度。
    Minimal,
    /// 低 reasoning 强度。
    Low,
    /// 中等 reasoning 强度。
    Medium,
    /// 高 reasoning 强度。
    High,
    /// 超高 reasoning 强度。
    XHigh,
    /// 最大 reasoning 强度。
    Max,
}

impl ReasoningLevel {
    /// 将协议中的 wire 字符串解析为目录枚举。
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    /// 返回标准下游协议使用的 wire 字符串。
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 一个标准下游 reasoning level 到 Upstream API wire level 的显式映射。
pub struct ReasoningLevelMapping {
    /// Public Model 已声明支持的标准下游 level。
    pub downstream: ReasoningLevel,
    /// 选定 Upstream API 实际接受的安全 wire 值。
    pub upstream: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// 模型输入和输出上下文长度的独立上限。
pub struct ModelContextLength {
    /// 已知的最大输入 token 数；`None` 表示未知。
    input_tokens: Option<u32>,
    /// 已知的最大输出 token 数；`None` 表示未知。
    output_tokens: Option<u32>,
}

impl ModelContextLength {
    /// 创建一组可独立未知的上下文长度限制。
    pub const fn new(input_tokens: Option<u32>, output_tokens: Option<u32>) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    /// 返回最大输入 token 数。
    pub const fn input_tokens(self) -> Option<u32> {
        self.input_tokens
    }

    /// 返回最大输出 token 数。
    pub const fn output_tokens(self) -> Option<u32> {
        self.output_tokens
    }
}

/// canonical Model 的任务模式。
///
/// 当前 OpenBridge 只注册可用于 Chat Completions/Responses 生成面的 `Chat` 模型；该枚举
/// 预留给未来模型信息投影，尚未参与 registry capability 计算。
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelMode {
    /// 对话式文本/多模态生成模型。
    Chat,
}

/// canonical Model 可接受的输入模态。
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputModality {
    /// Text input。
    Text,
    /// Image input。
    Image,
    /// Audio input。
    Audio,
    /// File input。
    File,
}

/// canonical Model 可生成的输出模态。
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputModality {
    /// Text output。
    Text,
    /// Image output。
    Image,
    /// Audio output。
    Audio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 与 Provider 无关的 canonical 模型事实。
pub struct ModelConfig {
    /// 目录内部稳定的模型 id。
    pub id: String,
    /// 给客户端展示的模型名称。
    pub name: String,
    /// 可选的模型描述。
    pub description: Option<String>,
    /// 模型本身声明的上下文长度。
    pub context_length: ModelContextLength,
    /// 已确认的模型任务模式；`None` 表示当前定义尚未提供证据。
    pub mode: Option<ModelMode>,
    /// 已确认的输入模态；`None` 表示未知，不能解释为空集合或明确不支持。
    pub input_modalities: Option<Vec<InputModality>>,
    /// 已确认的输出模态；`None` 表示未知，不能解释为空集合或明确不支持。
    pub output_modalities: Option<Vec<OutputModality>>,
    /// 模型支持的 OpenAI-compatible 参数名。
    pub supported_parameters: Vec<String>,
    /// 模型 reasoning 支持状态。
    pub reasoning: ReasoningSupport,
    /// 模型接受的 reasoning 强度集合。
    pub reasoning_levels: Vec<ReasoningLevel>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// Upstream API 对 canonical 模型事实施加的收窄规则。
pub struct UpstreamApiModelRules {
    /// Upstream API 可进一步收紧的上下文长度。
    pub context_length: ModelContextLength,
    /// Upstream API 可进一步收紧的 reasoning 状态。
    pub reasoning: Option<ReasoningSupport>,
    /// Upstream API 禁用但不能新增的参数名。
    pub disabled_parameters: Vec<String>,
    /// 标准下游 reasoning level 到该 Upstream API wire 值的显式映射。
    pub reasoning_level_mappings: Vec<ReasoningLevelMapping>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 一个 Provider 共享的 credential pool 声明。
pub struct CredentialPoolConfig {
    /// 注册表中的 pool id。
    pub id: String,
    /// 允许消费该 pool 的 Provider。
    pub provider: ProviderKind,
    /// adapter 支持的 credential 类型。
    pub kind: CredentialKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// 与具体协议绑定的 Upstream API capability 配置。
pub enum UpstreamApiCapabilities {
    /// Chat Completions endpoint 的能力。
    ChatCompletions(ChatCompletionsCapabilities),
    /// Responses endpoint 的能力。
    Responses(ResponsesCapabilities),
}

impl UpstreamApiCapabilities {
    /// 返回该 capability 配置对应的原生协议。
    pub const fn protocol(self) -> ApiProtocol {
        match self {
            Self::ChatCompletions(_) => ApiProtocol::ChatCompletions,
            Self::Responses(_) => ApiProtocol::Responses,
        }
    }

    /// 返回不包含 Responses 专有状态的协议公共能力。
    pub const fn generation_capabilities(self) -> GenerationCapabilities {
        match self {
            Self::ChatCompletions(capabilities) => capabilities.generation_capabilities(),
            Self::Responses(capabilities) => capabilities.generation_capabilities(),
        }
    }

    /// 如果这是 Responses 配置，则返回其完整能力。
    pub const fn responses(self) -> Option<ResponsesCapabilities> {
        match self {
            Self::ChatCompletions(_) => None,
            Self::Responses(capabilities) => Some(capabilities),
        }
    }

    /// 返回该 Upstream API 已声明的 reasoning 输出类型。
    pub const fn reasoning_output(self) -> crate::core::ReasoningOutput {
        match self {
            Self::ChatCompletions(capabilities) => capabilities.reasoning_output,
            Self::Responses(capabilities) => capabilities.reasoning_output,
        }
    }

    pub(super) fn is_subset_of(self, upper: ApiCapabilities) -> bool {
        match self {
            Self::ChatCompletions(capabilities) => {
                capabilities.is_subset_of(upper.chat_completions)
            }
            Self::Responses(capabilities) => capabilities.is_subset_of(upper.responses),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// 当前支持的上游 transport profile。
pub enum TransportKind {
    /// HTTP JSON 请求和 SSE response body。
    HttpJsonSse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// provider-issued continuation state 的归属范围。
pub enum StateAffinity {
    /// 请求不携带必须固定 target 的状态。
    Unbound,
    /// 状态绑定到当前 Upstream Target，禁止跨 target fallback。
    TargetBound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 一个 target 对外提供的原生 Upstream API。
pub struct UpstreamApiConfig {
    /// target 内稳定的 Upstream API id。
    pub id: String,
    /// Upstream API 原生提供的协议。
    pub protocol: ApiProtocol,
    /// 发往上游的真实模型 id。
    pub upstream_model: String,
    /// provider 允许的 endpoint profile。
    pub endpoint_profile: String,
    /// 当前原生 transport profile。
    pub transport: TransportKind,
    /// 对 Model 事实的 Upstream API 级收窄规则。
    pub model_rules: UpstreamApiModelRules,
    /// 单协议能力证据。
    pub capabilities: UpstreamApiCapabilities,
    /// continuation/state 所有权策略。
    pub state_affinity: StateAffinity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 可被 route 选择的受信上游 target。
pub struct UpstreamTargetConfig {
    /// 注册表中的 target id。
    pub id: String,
    /// 编译期 Provider Family。
    pub provider: ProviderKind,
    /// 引用的 Model id。
    pub model: String,
    /// 经过校验的 HTTPS endpoint base。
    pub base_url: String,
    /// target 引用的共享 credential pool id。
    pub credential_pool: String,
    /// 可选的明确共享 quota scope。
    pub quota_scope: Option<String>,
    /// 可选的故障/cooldown 域。
    pub fault_domain: Option<String>,
    /// 单次上游请求超时时间。
    pub request_timeout: Duration,
    /// 是否允许新的无状态请求选择该 target。
    pub enabled: bool,
    /// target 原生提供的协议级供应。
    pub upstream_apis: Vec<UpstreamApiConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// route 的请求处理模式。
pub enum RouteMode {
    /// 保持下游协议和上游协议原生一致。
    Native,
    /// 在两个 OpenAI-compatible 协议之间执行显式受限转换。
    Bridged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 将下游协议绑定到一个 Upstream API 的 route。
pub struct RouteConfig {
    /// 注册表中的 route id。
    pub id: String,
    /// 被 route 引用的 Upstream Target id。
    pub upstream_target: String,
    /// 被 route 引用的 Upstream API id。
    pub upstream_api: String,
    /// route 接受的下游原生协议。
    pub downstream_protocol: ApiProtocol,
    /// route 的处理模式。
    pub mode: RouteMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 向下游公开的模型及其有序 route 候选。
pub struct PublicModelConfig {
    /// 对下游公开的稳定 model name。
    pub name: String,
    /// 按优先级排列的完整 Route id。
    pub routes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 启动时编译 registry 所需的完整定义。
pub struct RegistryConfig {
    /// 用于报告和审计的注册表版本。
    pub version: String,
    /// 完整模型定义集合。
    pub models: Vec<ModelConfig>,
    /// 完整 credential pool 定义集合。
    pub credential_pools: Vec<CredentialPoolConfig>,
    /// 完整 Upstream Target 定义集合。
    pub upstream_targets: Vec<UpstreamTargetConfig>,
    /// 完整 Route 定义集合。
    pub routes: Vec<RouteConfig>,
    /// 完整 Public Model 定义集合。
    pub public_models: Vec<PublicModelConfig>,
}
