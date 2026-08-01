//! 编译期 Model、Upstream Target、Upstream API、Public Model 与 Route 注册表。
//!
//! 模型事实由 `src/models/*` 构造，target/upstream API/route 由 `src/providers/*` 构造。
//! 启动时 builder 对完整定义执行引用、能力和安全边界校验，成功后生成请求路径只读的
//! `RuntimeRegistry`。

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use thiserror::Error;
use url::Url;

use crate::{
    config::{BootstrapConfig, HttpClientConfig, RuntimeLimits},
    core::{ApiCapabilities, ApiProtocol, EndpointCapabilities, ResponsesCapabilities},
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
/// Upstream Target 使用的 credential binding 声明。
pub struct CredentialConfig {
    /// 注册表中的 credential id。
    pub id: String,
    /// adapter 支持的 credential 类型。
    pub kind: CredentialKind,
    /// 运行时读取 secret 的环境变量名。
    pub environment_variable: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// 与具体协议绑定的 Upstream API capability 配置。
pub enum UpstreamApiCapabilities {
    /// Chat Completions endpoint 的能力。
    ChatCompletions(EndpointCapabilities),
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
    pub const fn protocol_capabilities(self) -> EndpointCapabilities {
        match self {
            Self::ChatCompletions(capabilities) => capabilities,
            Self::Responses(capabilities) => capabilities.protocol_capabilities(),
        }
    }

    /// 如果这是 Responses 配置，则返回其完整能力。
    pub const fn responses(self) -> Option<ResponsesCapabilities> {
        match self {
            Self::ChatCompletions(_) => None,
            Self::Responses(capabilities) => Some(capabilities),
        }
    }

    const fn is_subset_of(self, upper: ApiCapabilities) -> bool {
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
    /// target 使用的 credential binding。
    pub credential: CredentialConfig,
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
    /// 完整 Upstream Target 定义集合。
    pub upstream_targets: Vec<UpstreamTargetConfig>,
    /// 完整 Route 定义集合。
    pub routes: Vec<RouteConfig>,
    /// 完整 Public Model 定义集合。
    pub public_models: Vec<PublicModelConfig>,
}

/// 编译期注册表定义不完整、引用不一致或尝试越权时返回的错误。
#[derive(Debug, Error)]
pub enum RegistryError {
    /// registry 版本为空。
    #[error("registry version must not be blank")]
    BlankVersion,
    /// 同一实体集合中存在重复 id。
    #[error("duplicate {entity} id '{id}'")]
    DuplicateId {
        /// 发生冲突的实体类型。
        entity: &'static str,
        /// 重复的实体 id。
        id: String,
    },
    /// 定义引用了不存在的实体。
    #[error("{entity} '{id}' references unknown {target} '{reference}'")]
    UnknownReference {
        /// 发起引用的实体类型。
        entity: &'static str,
        /// 发起引用的实体 id。
        id: String,
        /// 被引用的实体类型。
        target: &'static str,
        /// 未解析的引用值。
        reference: String,
    },
    /// credential locator 不是合法的环境变量名。
    #[error("upstream target '{upstream_target}' uses an invalid credential environment variable")]
    InvalidCredentialLocator {
        /// 使用非法 locator 的 target id。
        upstream_target: String,
    },
    /// target 选择了 provider 不支持的 credential 类型。
    #[error(
        "upstream target '{upstream_target}' uses a credential kind unsupported by its adapter"
    )]
    UnsupportedCredentialKind {
        /// 配置不兼容的 target id。
        upstream_target: String,
    },
    /// target endpoint 不是允许的 HTTPS base URL。
    #[error("upstream target '{upstream_target}' uses an invalid base URL")]
    InvalidBaseUrl {
        /// URL 不合法的 target id。
        upstream_target: String,
    },
    /// target 请求超时时间为零。
    #[error("upstream target '{upstream_target}' request timeout must be greater than zero")]
    InvalidRequestTimeout {
        /// 超时配置不合法的 target id。
        upstream_target: String,
    },
    /// target 没有声明任何 Upstream API。
    #[error("upstream target '{upstream_target}' must contain at least one upstream API")]
    EmptyUpstreamTarget {
        /// 没有 Upstream API 的 target id。
        upstream_target: String,
    },
    /// target 内存在重复 Upstream API id。
    #[error("upstream target '{upstream_target}' contains duplicate upstream API '{upstream_api}'")]
    DuplicateUpstreamApi {
        /// 发生冲突的 target id。
        upstream_target: String,
        /// 重复的 Upstream API id。
        upstream_api: String,
    },
    /// Upstream API 使用了 provider 未注册的 endpoint profile。
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' uses unsupported endpoint profile '{profile}'"
    )]
    UnsupportedEndpointProfile {
        /// 所属 target id。
        upstream_target: String,
        /// 不兼容的 Upstream API id。
        upstream_api: String,
        /// 未注册的 endpoint profile。
        profile: String,
    },
    /// Upstream API 的上游 model id 为空。
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' upstream model must not be blank"
    )]
    BlankUpstreamModel {
        /// 所属 target id。
        upstream_target: String,
        /// model id 为空的 Upstream API id。
        upstream_api: String,
    },
    /// Upstream API 的 capability 枚举与协议不一致。
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' capability type does not match protocol"
    )]
    UpstreamApiProtocolMismatch {
        /// 所属 target id。
        upstream_target: String,
        /// 配置不一致的 Upstream API id。
        upstream_api: String,
    },
    /// canonical 模型的必填字符串为空。
    #[error("model '{model}' field '{field}' must not be blank")]
    BlankModelField {
        /// 不合法的模型 id。
        model: String,
        /// 为空的字段名。
        field: &'static str,
    },
    /// canonical 模型声明了零值上下文长度。
    #[error("model '{model}' context length '{limit}' must be greater than zero")]
    InvalidModelContextLength {
        /// 不合法的模型 id。
        model: String,
        /// 不合法的长度字段名。
        limit: &'static str,
    },
    /// canonical 模型参数名不符合受限 wire 名称格式。
    #[error("model '{model}' declares invalid supported parameter '{parameter}'")]
    InvalidSupportedParameter {
        /// 不合法的模型 id。
        model: String,
        /// 不合法的参数名。
        parameter: String,
    },
    /// canonical 模型重复声明了参数名。
    #[error("model '{model}' declares supported parameter '{parameter}' more than once")]
    DuplicateSupportedParameter {
        /// 重复参数所属的模型 id。
        model: String,
        /// 重复的参数名。
        parameter: String,
    },
    /// canonical 模型的 reasoning 状态与参数集合不一致。
    #[error("model '{model}' has inconsistent reasoning configuration: {detail}")]
    InconsistentReasoningConfig {
        /// 配置不一致的模型 id。
        model: String,
        /// 具体不一致原因。
        detail: &'static str,
    },
    /// Upstream API 模型规则声明了零值限制。
    #[error("upstream API '{upstream_api}' model rule '{field}' must be greater than zero")]
    InvalidUpstreamApiModelRule {
        /// 规则所属的 Upstream API 标识。
        upstream_api: String,
        /// 不合法的规则字段。
        field: &'static str,
    },
    /// Upstream API 模型限制超过了 canonical 模型上限。
    #[error("upstream API '{upstream_api}' model rule '{field}' exceeds the model limit")]
    UpstreamApiModelLimitExceedsModel {
        /// 规则所属的 Upstream API 标识。
        upstream_api: String,
        /// 超出上限的规则字段。
        field: &'static str,
    },
    /// Upstream API 模型规则扩大了 canonical 模型事实。
    #[error("upstream API '{upstream_api}' model rule '{field}' widens the model information")]
    UpstreamApiModelRuleWidensModel {
        /// 规则所属的 Upstream API 标识。
        upstream_api: String,
        /// 被扩大声明的字段。
        field: &'static str,
    },
    /// Upstream API 试图禁用模型未声明的参数。
    #[error("upstream API '{upstream_api}' model rule disables undeclared parameter '{parameter}'")]
    UpstreamApiModelRuleDisablesUnknownParameter {
        /// 规则所属的 Upstream API 标识。
        upstream_api: String,
        /// 未声明却被禁用的参数名。
        parameter: String,
    },
    /// Upstream API 收窄后的 reasoning 配置不一致。
    #[error("upstream API '{upstream_api}' model rules are inconsistent: {detail}")]
    InconsistentUpstreamApiModelRules {
        /// 规则所属的 Upstream API 标识。
        upstream_api: String,
        /// 具体不一致原因。
        detail: &'static str,
    },
    /// Upstream API 声明了超过 provider contract 的能力。
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' enables capabilities unsupported by its adapter"
    )]
    CapabilityElevation {
        /// 所属 target id。
        upstream_target: String,
        /// 越权声明能力的 Upstream API id。
        upstream_api: String,
    },
    /// Native route 的下游协议与 Upstream API 协议不一致。
    #[error("native route '{route}' protocol does not match its upstream API")]
    NativeRouteProtocolMismatch {
        /// 协议不匹配的 route id。
        route: String,
    },
    /// Bridged route 的下游协议与 Upstream API 协议相同。
    #[error("bridged route '{route}' must target the opposite upstream protocol")]
    BridgedRouteProtocolMatch {
        /// 协议方向无转换意义的 route id。
        route: String,
    },
    /// Public Model 重复引用同一个 route。
    #[error("public model '{public_model}' contains duplicate route '{route}'")]
    DuplicatePublicModelRoute {
        /// 发生冲突的 Public Model 名称。
        public_model: String,
        /// 重复的 route id。
        route: String,
    },
    /// Public Model 没有任何 route。
    #[error("public model '{public_model}' must contain at least one route")]
    EmptyPublicModel {
        /// 没有 route 的 Public Model 名称。
        public_model: String,
    },
}

/// 启动后供请求路径读取的模型元数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInfo {
    id: String,
    name: String,
    description: Option<String>,
    context_length: ModelContextLength,
    supported_parameters: Vec<String>,
    reasoning: ReasoningSupport,
    reasoning_levels: Vec<ReasoningLevel>,
}

impl ModelInfo {
    /// 返回稳定模型 id。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回展示名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回可选模型描述。
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// 返回生效后的上下文长度。
    pub const fn context_length(&self) -> ModelContextLength {
        self.context_length
    }

    /// 返回生效后的支持参数。
    pub fn supported_parameters(&self) -> &[String] {
        &self.supported_parameters
    }

    /// 返回生效后的 reasoning 状态。
    pub const fn reasoning(&self) -> ReasoningSupport {
        self.reasoning
    }

    /// 返回生效后的 reasoning 强度。
    pub fn reasoning_levels(&self) -> &[ReasoningLevel] {
        &self.reasoning_levels
    }
}

/// 启动后供请求路径读取的不可变 registry snapshot。
#[derive(Debug)]
pub struct RuntimeRegistry {
    version: RegistryVersion,
    bootstrap: BootstrapConfig,
    models: BTreeMap<String, ModelInfo>,
    upstream_targets: BTreeMap<String, UpstreamTarget>,
    routes: BTreeMap<String, Route>,
    public_models: BTreeMap<String, PublicModel>,
}

impl RuntimeRegistry {
    /// 返回编译期注册表版本。
    pub fn version(&self) -> &RegistryVersion {
        &self.version
    }

    /// 返回 bootstrap 的 loopback 监听地址。
    pub fn listen(&self) -> std::net::SocketAddr {
        self.bootstrap.listen()
    }

    /// 返回运行时资源限制。
    pub fn limits(&self) -> &RuntimeLimits {
        self.bootstrap.limits()
    }

    /// 返回上游 HTTP client 策略。
    pub fn http_client(&self) -> &HttpClientConfig {
        self.bootstrap.http_client()
    }

    /// 按内部模型 id 查询模型元数据。
    pub fn model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.get(id)
    }

    /// 按内部 target id 查询解析结果。
    pub fn upstream_target(&self, id: &str) -> Option<&UpstreamTarget> {
        self.upstream_targets.get(id)
    }

    /// 枚举所有内部 target id。
    pub fn upstream_target_ids(&self) -> impl Iterator<Item = &str> {
        self.upstream_targets.keys().map(String::as_str)
    }

    /// 按 route id 查询已解析的 route。
    pub fn route(&self, id: &str) -> Option<&Route> {
        self.routes.get(id)
    }

    /// 按下游公开名称查询 Public Model。
    pub fn public_model(&self, name: &str) -> Option<&PublicModel> {
        self.public_models.get(name)
    }

    /// 枚举下游 `/v1/models` 可公开的 Public Model。
    pub fn public_models(&self) -> impl Iterator<Item = &str> {
        self.public_models.keys().map(String::as_str)
    }
}

/// 已通过校验的 registry 版本标识。
#[derive(Debug)]
pub struct RegistryVersion(String);

impl RegistryVersion {
    /// 返回版本字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 已解析的 credential binding。
#[derive(Debug)]
pub struct CredentialBinding {
    id: String,
    kind: CredentialKind,
    secret_reference: SecretLocator,
}

impl CredentialBinding {
    /// 返回 credential binding id。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回 credential 类型。
    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    /// 返回不包含 secret 的受信 secret locator。
    pub fn secret_reference(&self) -> &SecretLocator {
        &self.secret_reference
    }
}

/// 已通过 endpoint、credential 和模型引用校验的上游 target。
#[derive(Debug)]
pub struct UpstreamTarget {
    kind: ProviderKind,
    credential: CredentialBinding,
    model_id: String,
    endpoint_base: Url,
    quota_scope: Option<String>,
    fault_domain: Option<String>,
    request_timeout: Duration,
    enabled: bool,
    upstream_apis: BTreeMap<String, UpstreamApi>,
}

impl UpstreamTarget {
    /// 返回 target 使用的 provider kind。
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// 返回 target 的 credential binding。
    pub fn credential(&self) -> &CredentialBinding {
        &self.credential
    }

    /// 返回 target 引用的 canonical 模型 id。
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// 返回经过校验的 endpoint base URL。
    pub fn endpoint_base(&self) -> &Url {
        &self.endpoint_base
    }

    /// 返回可选的共享 quota scope。
    pub fn quota_scope(&self) -> Option<&str> {
        self.quota_scope.as_deref()
    }

    /// 返回可选的故障隔离域。
    pub fn fault_domain(&self) -> Option<&str> {
        self.fault_domain.as_deref()
    }

    /// 返回单次上游请求超时时间。
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// 判断 target 是否允许新的无状态请求选择。
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 按 Upstream API id 查询已解析 API。
    pub fn upstream_api(&self, id: &str) -> Option<&UpstreamApi> {
        self.upstream_apis.get(id)
    }

    /// 按原生协议查找一个 Upstream API。
    pub fn upstream_api_for_protocol(&self, protocol: ApiProtocol) -> Option<&UpstreamApi> {
        self.upstream_apis
            .values()
            .find(|upstream_api| upstream_api.protocol() == protocol)
    }

    /// 枚举 target 下所有 Upstream API 及其 id。
    pub fn upstream_apis(&self) -> impl Iterator<Item = (&str, &UpstreamApi)> {
        self.upstream_apis
            .iter()
            .map(|(id, upstream_api)| (id.as_str(), upstream_api))
    }
}

/// 不暴露 secret 内容的 credential locator。
#[derive(Debug)]
pub struct SecretLocator {
    locator: String,
}

impl SecretLocator {
    /// 返回 locator scheme；当前固定为环境变量 `env`。
    pub fn scheme(&self) -> &'static str {
        "env"
    }

    /// 返回环境变量名。
    pub fn locator(&self) -> &str {
        &self.locator
    }
}

/// 已解析并应用模型规则的 Upstream API。
#[derive(Debug)]
pub struct UpstreamApi {
    protocol: ApiProtocol,
    model: ModelInfo,
    upstream_model: String,
    endpoint_profile: String,
    transport: TransportKind,
    capabilities: UpstreamApiCapabilities,
    state_affinity: StateAffinity,
    reasoning_level_mappings: BTreeMap<ReasoningLevel, String>,
}

impl UpstreamApi {
    /// 返回 Upstream API 的原生协议。
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    /// 返回应用规则后的模型元数据。
    pub fn model(&self) -> &ModelInfo {
        &self.model
    }

    /// 返回发送给上游的真实模型 id。
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// 返回 provider 识别用的 endpoint profile。
    pub fn endpoint_profile(&self) -> &str {
        &self.endpoint_profile
    }

    /// 返回使用的 transport profile。
    pub fn transport(&self) -> TransportKind {
        self.transport
    }

    /// 返回该 API 的协议能力。
    pub fn capabilities(&self) -> UpstreamApiCapabilities {
        self.capabilities
    }

    /// 返回 continuation state 的归属策略。
    pub fn state_affinity(&self) -> StateAffinity {
        self.state_affinity
    }

    /// 返回指定标准 level 在该 Upstream API 上的显式 wire 映射。
    pub fn reasoning_level_mapping(&self, level: ReasoningLevel) -> Option<&str> {
        self.reasoning_level_mappings
            .get(&level)
            .map(String::as_str)
    }
}

/// 已解析的 route 绑定关系。
#[derive(Debug)]
pub struct Route {
    upstream_target: String,
    upstream_api: String,
    downstream_protocol: ApiProtocol,
    mode: RouteMode,
}

impl Route {
    /// 返回 route 绑定的 Upstream Target id。
    pub fn upstream_target(&self) -> &str {
        &self.upstream_target
    }

    /// 返回 route 绑定的 Upstream API id。
    pub fn upstream_api(&self) -> &str {
        &self.upstream_api
    }

    /// 返回 route 接受的下游协议。
    pub fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_protocol
    }

    /// 返回 route 的处理模式。
    pub fn mode(&self) -> RouteMode {
        self.mode
    }
}

/// 已解析的下游 Public Model 及有序 route 列表。
#[derive(Debug)]
pub struct PublicModel {
    routes: Vec<String>,
}

impl PublicModel {
    /// 返回按优先级排列的 route id。
    pub fn routes(&self) -> &[String] {
        &self.routes
    }
}

/// 校验完整 registry 定义并构造请求路径只读的运行时 snapshot。
///
/// 校验阶段拒绝未知引用、重复 id、能力越权、非安全 endpoint 和不一致的模型收窄规则；
/// 成功后返回值不再依赖运行时配置注册新 provider 或 target。
pub fn build_registry(
    bootstrap: BootstrapConfig,
    definition: RegistryConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    // 校验版本并建立 canonical model 索引。
    if definition.version.trim().is_empty() {
        return Err(RegistryError::BlankVersion);
    }

    let mut models = BTreeMap::new();
    for model in definition.models {
        validate_model_config(&model)?;
        let id = model.id.clone();
        let resolved = ModelInfo {
            id: id.clone(),
            name: model.name,
            description: model.description,
            context_length: model.context_length,
            supported_parameters: model.supported_parameters,
            reasoning: model.reasoning,
            reasoning_levels: model.reasoning_levels,
        };
        if models.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "model",
                id,
            });
        }
    }

    // 校验 target、credential、endpoint 和 Upstream API，并解析模型收窄规则。
    let mut credential_ids = BTreeSet::new();
    let mut upstream_targets = BTreeMap::new();
    for target in definition.upstream_targets {
        if !target
            .provider
            .accepts_credential_kind(target.credential.kind)
        {
            return Err(RegistryError::UnsupportedCredentialKind {
                upstream_target: target.id,
            });
        }
        if !is_valid_environment_variable(&target.credential.environment_variable) {
            return Err(RegistryError::InvalidCredentialLocator {
                upstream_target: target.id,
            });
        }
        if !credential_ids.insert(target.credential.id.clone()) {
            return Err(RegistryError::DuplicateId {
                entity: "credential",
                id: target.credential.id,
            });
        }
        let model =
            models
                .get(&target.model)
                .cloned()
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "upstream target",
                    id: target.id.clone(),
                    target: "real model",
                    reference: target.model.clone(),
                })?;
        if target.request_timeout.is_zero() {
            return Err(RegistryError::InvalidRequestTimeout {
                upstream_target: target.id,
            });
        }
        if target.upstream_apis.is_empty() {
            return Err(RegistryError::EmptyUpstreamTarget {
                upstream_target: target.id,
            });
        }
        let endpoint_base = normalize_endpoint_base(&target.base_url).ok_or_else(|| {
            RegistryError::InvalidBaseUrl {
                upstream_target: target.id.clone(),
            }
        })?;
        let mut upstream_apis = BTreeMap::new();
        for upstream_api in target.upstream_apis {
            if upstream_api.protocol != upstream_api.capabilities.protocol() {
                return Err(RegistryError::UpstreamApiProtocolMismatch {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
            if upstream_api.upstream_model.trim().is_empty() {
                return Err(RegistryError::BlankUpstreamModel {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
            if !target
                .provider
                .accepts_endpoint_profile(&upstream_api.endpoint_profile)
            {
                return Err(RegistryError::UnsupportedEndpointProfile {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                    profile: upstream_api.endpoint_profile,
                });
            }
            if !upstream_api
                .capabilities
                .is_subset_of(target.provider.capabilities())
            {
                return Err(RegistryError::CapabilityElevation {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
            let api_key = format!("{}/{}", target.id, upstream_api.id);
            let mapping_config = upstream_api.model_rules.reasoning_level_mappings.clone();
            let effective_model =
                apply_model_rules(model.clone(), &api_key, upstream_api.model_rules)?;
            let reasoning_level_mappings =
                validate_reasoning_level_mappings(&api_key, &effective_model, mapping_config)?;
            let resolved = UpstreamApi {
                protocol: upstream_api.protocol,
                model: effective_model,
                upstream_model: upstream_api.upstream_model,
                endpoint_profile: upstream_api.endpoint_profile,
                transport: upstream_api.transport,
                capabilities: upstream_api.capabilities,
                state_affinity: upstream_api.state_affinity,
                reasoning_level_mappings,
            };
            if upstream_apis
                .insert(upstream_api.id.clone(), resolved)
                .is_some()
            {
                return Err(RegistryError::DuplicateUpstreamApi {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
        }
        let resolved = UpstreamTarget {
            kind: target.provider,
            credential: CredentialBinding {
                id: target.credential.id,
                kind: target.credential.kind,
                secret_reference: SecretLocator {
                    locator: target.credential.environment_variable,
                },
            },
            model_id: target.model,
            endpoint_base,
            quota_scope: target.quota_scope,
            fault_domain: target.fault_domain,
            request_timeout: target.request_timeout,
            enabled: target.enabled,
            upstream_apis,
        };
        if upstream_targets
            .insert(target.id.clone(), resolved)
            .is_some()
        {
            return Err(RegistryError::DuplicateId {
                entity: "upstream target",
                id: target.id,
            });
        }
    }

    // 校验 route 引用及 native 协议一致性。
    let mut routes = BTreeMap::new();
    for route in definition.routes {
        let target = upstream_targets
            .get(&route.upstream_target)
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "route",
                id: route.id.clone(),
                target: "upstream target",
                reference: route.upstream_target.clone(),
            })?;
        let upstream_api = target.upstream_api(&route.upstream_api).ok_or_else(|| {
            RegistryError::UnknownReference {
                entity: "route",
                id: route.id.clone(),
                target: "upstream API",
                reference: format!("{}/{}", route.upstream_target, route.upstream_api),
            }
        })?;
        if route.mode == RouteMode::Native && route.downstream_protocol != upstream_api.protocol() {
            return Err(RegistryError::NativeRouteProtocolMismatch { route: route.id });
        }
        if route.mode == RouteMode::Bridged && route.downstream_protocol == upstream_api.protocol()
        {
            return Err(RegistryError::BridgedRouteProtocolMatch { route: route.id });
        }
        let resolved = Route {
            upstream_target: route.upstream_target,
            upstream_api: route.upstream_api,
            downstream_protocol: route.downstream_protocol,
            mode: route.mode,
        };
        if routes.insert(route.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "route",
                id: route.id,
            });
        }
    }

    // 校验 Public Model 的 route 顺序、唯一性和完整引用。
    let mut public_models = BTreeMap::new();
    for public_model in definition.public_models {
        if public_model.routes.is_empty() {
            return Err(RegistryError::EmptyPublicModel {
                public_model: public_model.name,
            });
        }
        let mut seen = BTreeSet::new();
        for route in &public_model.routes {
            if !seen.insert(route) {
                return Err(RegistryError::DuplicatePublicModelRoute {
                    public_model: public_model.name,
                    route: route.clone(),
                });
            }
            if !routes.contains_key(route) {
                return Err(RegistryError::UnknownReference {
                    entity: "public model",
                    id: public_model.name,
                    target: "route",
                    reference: route.clone(),
                });
            }
        }
        if public_models
            .insert(
                public_model.name.clone(),
                PublicModel {
                    routes: public_model.routes,
                },
            )
            .is_some()
        {
            return Err(RegistryError::DuplicateId {
                entity: "public model",
                id: public_model.name,
            });
        }
    }

    // 固化所有解析结果为请求路径只读 snapshot。
    Ok(RuntimeRegistry {
        version: RegistryVersion(definition.version),
        bootstrap,
        models,
        upstream_targets,
        routes,
        public_models,
    })
}

/// 校验 canonical 模型字段、参数名称和 reasoning 配置的一致性。
fn validate_model_config(model: &ModelConfig) -> Result<(), RegistryError> {
    for (field, value) in [("id", model.id.as_str()), ("name", model.name.as_str())] {
        if value.trim().is_empty() {
            return Err(RegistryError::BlankModelField {
                model: model.id.clone(),
                field,
            });
        }
    }
    if model
        .description
        .as_deref()
        .is_some_and(|description| description.trim().is_empty())
    {
        return Err(RegistryError::BlankModelField {
            model: model.id.clone(),
            field: "description",
        });
    }
    for (limit, value) in [
        ("input", model.context_length.input_tokens()),
        ("output", model.context_length.output_tokens()),
    ] {
        if value == Some(0) {
            return Err(RegistryError::InvalidModelContextLength {
                model: model.id.clone(),
                limit,
            });
        }
    }
    let mut seen = BTreeSet::new();
    for parameter in &model.supported_parameters {
        if !is_valid_parameter_name(parameter) {
            return Err(RegistryError::InvalidSupportedParameter {
                model: model.id.clone(),
                parameter: parameter.clone(),
            });
        }
        if !seen.insert(parameter) {
            return Err(RegistryError::DuplicateSupportedParameter {
                model: model.id.clone(),
                parameter: parameter.clone(),
            });
        }
    }
    validate_reasoning_config(&model.id, &model.supported_parameters, model.reasoning).and_then(
        |()| validate_reasoning_levels(&model.id, model.reasoning, &model.reasoning_levels),
    )
}

/// 校验 reasoning level 只在 supported 状态下出现且不重复。
fn validate_reasoning_levels(
    model: &str,
    reasoning: ReasoningSupport,
    levels: &[ReasoningLevel],
) -> Result<(), RegistryError> {
    if reasoning != ReasoningSupport::Supported && !levels.is_empty() {
        return Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning levels require reasoning = supported",
        });
    }
    let mut seen = BTreeSet::new();
    if levels.iter().any(|level| !seen.insert(*level)) {
        return Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning levels must not contain duplicates",
        });
    }
    Ok(())
}

/// 校验 reasoning 状态与模型支持参数集合的一致性。
fn validate_reasoning_config(
    model: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning = supported requires supported_parameters to include reasoning",
        }),
        (ReasoningSupport::Unsupported, true) => Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning = unsupported conflicts with supported_parameters",
        }),
        _ => Ok(()),
    }
}

/// 将 Upstream API 的收窄规则应用到 canonical 模型事实。
fn apply_model_rules(
    model: ModelInfo,
    upstream_api: &str,
    rules: UpstreamApiModelRules,
) -> Result<ModelInfo, RegistryError> {
    // 先验证上下文长度规则不会扩大 canonical 模型上限。
    validate_model_limit(
        upstream_api,
        "context_length.input",
        model.context_length.input_tokens(),
        rules.context_length.input_tokens(),
    )?;
    validate_model_limit(
        upstream_api,
        "context_length.output",
        model.context_length.output_tokens(),
        rules.context_length.output_tokens(),
    )?;
    // 计算 reasoning 收窄结果并拒绝能力扩大。
    let reasoning = rules.reasoning.unwrap_or(model.reasoning);
    if reasoning_rank(reasoning) > reasoning_rank(model.reasoning) {
        return Err(RegistryError::UpstreamApiModelRuleWidensModel {
            upstream_api: upstream_api.to_owned(),
            field: "reasoning",
        });
    }
    // 应用参数禁用集合，并拒绝禁用模型未声明的参数。
    let disabled = rules.disabled_parameters.iter().collect::<BTreeSet<_>>();
    for parameter in &disabled {
        if !model.supported_parameters.contains(parameter) {
            return Err(
                RegistryError::UpstreamApiModelRuleDisablesUnknownParameter {
                    upstream_api: upstream_api.to_owned(),
                    parameter: (*parameter).clone(),
                },
            );
        }
    }
    // 构造有效参数集合并重新验证 reasoning 语义。
    let supported_parameters = model
        .supported_parameters
        .iter()
        .filter(|parameter| !disabled.contains(parameter))
        .cloned()
        .collect::<Vec<_>>();
    validate_effective_reasoning_config(upstream_api, &supported_parameters, reasoning)?;
    Ok(ModelInfo {
        id: model.id,
        name: model.name,
        description: model.description,
        context_length: ModelContextLength::new(
            min_known_limit(
                model.context_length.input_tokens(),
                rules.context_length.input_tokens(),
            ),
            min_known_limit(
                model.context_length.output_tokens(),
                rules.context_length.output_tokens(),
            ),
        ),
        supported_parameters,
        reasoning,
        reasoning_levels: if reasoning == ReasoningSupport::Supported {
            model.reasoning_levels
        } else {
            Vec::new()
        },
    })
}

/// 校验 Upstream API 的单项模型限制为正且不超过 canonical 上限。
fn validate_model_limit(
    upstream_api: &str,
    field: &'static str,
    model_limit: Option<u32>,
    constraint_limit: Option<u32>,
) -> Result<(), RegistryError> {
    let Some(constraint_limit) = constraint_limit else {
        return Ok(());
    };
    if constraint_limit == 0 {
        return Err(RegistryError::InvalidUpstreamApiModelRule {
            upstream_api: upstream_api.to_owned(),
            field,
        });
    }
    if model_limit.is_some_and(|model_limit| constraint_limit > model_limit) {
        return Err(RegistryError::UpstreamApiModelLimitExceedsModel {
            upstream_api: upstream_api.to_owned(),
            field,
        });
    }
    Ok(())
}

/// 校验应用收窄规则后的 reasoning 状态和参数集合。
fn validate_effective_reasoning_config(
    upstream_api: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => {
            Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning = supported requires the effective parameter set to include reasoning",
            })
        }
        (ReasoningSupport::Unsupported, true) => {
            Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning = unsupported conflicts with the effective parameter set",
            })
        }
        _ => Ok(()),
    }
}

/// 校验映射不会扩大 canonical reasoning 契约，并编译为唯一源 level 的只读表。
fn validate_reasoning_level_mappings(
    upstream_api: &str,
    model: &ModelInfo,
    mappings: Vec<ReasoningLevelMapping>,
) -> Result<BTreeMap<ReasoningLevel, String>, RegistryError> {
    // 逐项校验源 level 已由有效模型声明，目标是受限 wire 名称。
    let mut resolved = BTreeMap::new();
    for mapping in mappings {
        if model.reasoning() != ReasoningSupport::Supported
            || !model.reasoning_levels().contains(&mapping.downstream)
        {
            return Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning level mapping source must be supported by the effective model",
            });
        }
        if !is_valid_parameter_name(&mapping.upstream) {
            return Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning level mapping target must be a safe wire name",
            });
        }

        // 同一源 level 只能映射到一个确定目标，避免候选内出现歧义。
        if resolved
            .insert(mapping.downstream, mapping.upstream)
            .is_some()
        {
            return Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning level mapping sources must be unique",
            });
        }
    }
    Ok(resolved)
}

/// 合并两个可选限制，并取已知限制中的较小值。
fn min_known_limit(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

/// 将 reasoning 状态映射为可比较的保守性等级。
fn reasoning_rank(reasoning: ReasoningSupport) -> u8 {
    match reasoning {
        ReasoningSupport::Unsupported => 0,
        ReasoningSupport::Unknown => 1,
        ReasoningSupport::Supported => 2,
    }
}

/// 判断模型参数名是否符合内部的受限小写 wire 名称格式。
fn is_valid_parameter_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// 判断 credential locator 是否为合法环境变量名。
fn is_valid_environment_variable(locator: &str) -> bool {
    let mut characters = locator.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

/// 校验并规范化只允许 HTTPS、无凭据和安全 path 前缀的 endpoint base。
fn normalize_endpoint_base(value: &str) -> Option<Url> {
    let mut url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !is_safe_endpoint_prefix(url.path())
    {
        return None;
    }
    if url.path() != "/" && !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Some(url)
}

/// 拒绝 endpoint path 中的 authority 绕过、空段和 dot-segment。
fn is_safe_endpoint_prefix(path: &str) -> bool {
    if path.is_empty() || path == "/" {
        return true;
    }
    if !path.starts_with('/') || path.contains("//") {
        return false;
    }
    path.trim_matches('/').split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
            })
    })
}
