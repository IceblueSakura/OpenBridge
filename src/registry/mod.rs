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
pub enum ReasoningLevel {
    /// 最低 reasoning 强度。
    Minimal,
    /// 低 reasoning 强度。
    Low,
    /// 中等 reasoning 强度。
    Medium,
    /// 高 reasoning 强度。
    High,
    /// 最高 reasoning 强度。
    XHigh,
}

impl ReasoningLevel {
    /// 将协议中的 wire 字符串解析为目录枚举。
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
pub struct UpstreamApiModelRules {
    /// Upstream API 可进一步收紧的上下文长度。
    pub context_length: ModelContextLength,
    /// Upstream API 可进一步收紧的 reasoning 状态。
    pub reasoning: Option<ReasoningSupport>,
    /// Upstream API 禁用但不能新增的参数名。
    pub disabled_parameters: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialConfig {
    /// 注册表中的 credential id。
    pub id: String,
    /// adapter 支持的 credential 类型。
    pub kind: CredentialKind,
    /// 运行时读取 secret 的环境变量名。
    pub environment_variable: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamApiCapabilities {
    ChatCompletions(EndpointCapabilities),
    Responses(ResponsesCapabilities),
}

impl UpstreamApiCapabilities {
    pub const fn protocol(self) -> ApiProtocol {
        match self {
            Self::ChatCompletions(_) => ApiProtocol::ChatCompletions,
            Self::Responses(_) => ApiProtocol::Responses,
        }
    }

    pub const fn protocol_capabilities(self) -> EndpointCapabilities {
        match self {
            Self::ChatCompletions(capabilities) => capabilities,
            Self::Responses(capabilities) => capabilities.protocol_capabilities(),
        }
    }

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
pub enum TransportKind {
    HttpJsonSse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateAffinity {
    Unbound,
    TargetBound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
pub enum RouteMode {
    Native,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteConfig {
    pub id: String,
    pub upstream_target: String,
    pub upstream_api: String,
    pub downstream_protocol: ApiProtocol,
    pub mode: RouteMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicModelConfig {
    /// 对下游公开的稳定 model name。
    pub name: String,
    /// 按优先级排列的完整 Route id。
    pub routes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    #[error("registry version must not be blank")]
    BlankVersion,
    #[error("duplicate {entity} id '{id}'")]
    DuplicateId { entity: &'static str, id: String },
    #[error("{entity} '{id}' references unknown {target} '{reference}'")]
    UnknownReference {
        entity: &'static str,
        id: String,
        target: &'static str,
        reference: String,
    },
    #[error("upstream target '{upstream_target}' uses an invalid credential environment variable")]
    InvalidCredentialLocator { upstream_target: String },
    #[error(
        "upstream target '{upstream_target}' uses a credential kind unsupported by its adapter"
    )]
    UnsupportedCredentialKind { upstream_target: String },
    #[error("upstream target '{upstream_target}' uses an invalid base URL")]
    InvalidBaseUrl { upstream_target: String },
    #[error("upstream target '{upstream_target}' request timeout must be greater than zero")]
    InvalidRequestTimeout { upstream_target: String },
    #[error("upstream target '{upstream_target}' must contain at least one upstream API")]
    EmptyUpstreamTarget { upstream_target: String },
    #[error("upstream target '{upstream_target}' contains duplicate upstream API '{upstream_api}'")]
    DuplicateUpstreamApi {
        upstream_target: String,
        upstream_api: String,
    },
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' uses unsupported endpoint profile '{profile}'"
    )]
    UnsupportedEndpointProfile {
        upstream_target: String,
        upstream_api: String,
        profile: String,
    },
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' upstream model must not be blank"
    )]
    BlankUpstreamModel {
        upstream_target: String,
        upstream_api: String,
    },
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' capability type does not match protocol"
    )]
    UpstreamApiProtocolMismatch {
        upstream_target: String,
        upstream_api: String,
    },
    #[error("model '{model}' field '{field}' must not be blank")]
    BlankModelField { model: String, field: &'static str },
    #[error("model '{model}' context length '{limit}' must be greater than zero")]
    InvalidModelContextLength { model: String, limit: &'static str },
    #[error("model '{model}' declares invalid supported parameter '{parameter}'")]
    InvalidSupportedParameter { model: String, parameter: String },
    #[error("model '{model}' declares supported parameter '{parameter}' more than once")]
    DuplicateSupportedParameter { model: String, parameter: String },
    #[error("model '{model}' has inconsistent reasoning configuration: {detail}")]
    InconsistentReasoningConfig { model: String, detail: &'static str },
    #[error("upstream API '{upstream_api}' model rule '{field}' must be greater than zero")]
    InvalidUpstreamApiModelRule {
        upstream_api: String,
        field: &'static str,
    },
    #[error("upstream API '{upstream_api}' model rule '{field}' exceeds the model limit")]
    UpstreamApiModelLimitExceedsModel {
        upstream_api: String,
        field: &'static str,
    },
    #[error("upstream API '{upstream_api}' model rule '{field}' widens the model information")]
    UpstreamApiModelRuleWidensModel {
        upstream_api: String,
        field: &'static str,
    },
    #[error("upstream API '{upstream_api}' model rule disables undeclared parameter '{parameter}'")]
    UpstreamApiModelRuleDisablesUnknownParameter {
        upstream_api: String,
        parameter: String,
    },
    #[error("upstream API '{upstream_api}' model rules are inconsistent: {detail}")]
    InconsistentUpstreamApiModelRules {
        upstream_api: String,
        detail: &'static str,
    },
    #[error(
        "upstream API '{upstream_api}' on upstream target '{upstream_target}' enables capabilities unsupported by its adapter"
    )]
    CapabilityElevation {
        upstream_target: String,
        upstream_api: String,
    },
    #[error("native route '{route}' protocol does not match its upstream API")]
    NativeRouteProtocolMismatch { route: String },
    #[error("public model '{public_model}' contains duplicate route '{route}'")]
    DuplicatePublicModelRoute { public_model: String, route: String },
    #[error("public model '{public_model}' must contain at least one route")]
    EmptyPublicModel { public_model: String },
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

    pub fn route(&self, id: &str) -> Option<&Route> {
        self.routes.get(id)
    }

    pub fn public_model(&self, name: &str) -> Option<&PublicModel> {
        self.public_models.get(name)
    }

    /// 枚举下游 `/v1/models` 可公开的 Public Model。
    pub fn public_models(&self) -> impl Iterator<Item = &str> {
        self.public_models.keys().map(String::as_str)
    }
}

#[derive(Debug)]
pub struct RegistryVersion(String);

impl RegistryVersion {
    /// 返回版本字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct CredentialBinding {
    id: String,
    kind: CredentialKind,
    secret_reference: SecretLocator,
}

impl CredentialBinding {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub fn secret_reference(&self) -> &SecretLocator {
        &self.secret_reference
    }
}

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
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn credential(&self) -> &CredentialBinding {
        &self.credential
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn endpoint_base(&self) -> &Url {
        &self.endpoint_base
    }

    pub fn quota_scope(&self) -> Option<&str> {
        self.quota_scope.as_deref()
    }

    pub fn fault_domain(&self) -> Option<&str> {
        self.fault_domain.as_deref()
    }

    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn upstream_api(&self, id: &str) -> Option<&UpstreamApi> {
        self.upstream_apis.get(id)
    }

    pub fn upstream_api_for_protocol(&self, protocol: ApiProtocol) -> Option<&UpstreamApi> {
        self.upstream_apis
            .values()
            .find(|upstream_api| upstream_api.protocol() == protocol)
    }

    pub fn upstream_apis(&self) -> impl Iterator<Item = (&str, &UpstreamApi)> {
        self.upstream_apis
            .iter()
            .map(|(id, upstream_api)| (id.as_str(), upstream_api))
    }
}

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

#[derive(Debug)]
pub struct UpstreamApi {
    protocol: ApiProtocol,
    model: ModelInfo,
    upstream_model: String,
    endpoint_profile: String,
    transport: TransportKind,
    capabilities: UpstreamApiCapabilities,
    state_affinity: StateAffinity,
}

impl UpstreamApi {
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    pub fn model(&self) -> &ModelInfo {
        &self.model
    }

    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub fn endpoint_profile(&self) -> &str {
        &self.endpoint_profile
    }

    pub fn transport(&self) -> TransportKind {
        self.transport
    }

    pub fn capabilities(&self) -> UpstreamApiCapabilities {
        self.capabilities
    }

    pub fn state_affinity(&self) -> StateAffinity {
        self.state_affinity
    }
}

#[derive(Debug)]
pub struct Route {
    upstream_target: String,
    upstream_api: String,
    downstream_protocol: ApiProtocol,
    mode: RouteMode,
}

impl Route {
    pub fn upstream_target(&self) -> &str {
        &self.upstream_target
    }

    pub fn upstream_api(&self) -> &str {
        &self.upstream_api
    }

    pub fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_protocol
    }

    pub fn mode(&self) -> RouteMode {
        self.mode
    }
}

#[derive(Debug)]
pub struct PublicModel {
    routes: Vec<String>,
}

impl PublicModel {
    pub fn routes(&self) -> &[String] {
        &self.routes
    }
}

pub fn build_registry(
    bootstrap: BootstrapConfig,
    definition: RegistryConfig,
) -> Result<RuntimeRegistry, RegistryError> {
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
            let effective_model =
                apply_model_rules(model.clone(), &api_key, upstream_api.model_rules)?;
            let resolved = UpstreamApi {
                protocol: upstream_api.protocol,
                model: effective_model,
                upstream_model: upstream_api.upstream_model,
                endpoint_profile: upstream_api.endpoint_profile,
                transport: upstream_api.transport,
                capabilities: upstream_api.capabilities,
                state_affinity: upstream_api.state_affinity,
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

    Ok(RuntimeRegistry {
        version: RegistryVersion(definition.version),
        bootstrap,
        models,
        upstream_targets,
        routes,
        public_models,
    })
}

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

fn apply_model_rules(
    model: ModelInfo,
    upstream_api: &str,
    rules: UpstreamApiModelRules,
) -> Result<ModelInfo, RegistryError> {
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
    let reasoning = rules.reasoning.unwrap_or(model.reasoning);
    if reasoning_rank(reasoning) > reasoning_rank(model.reasoning) {
        return Err(RegistryError::UpstreamApiModelRuleWidensModel {
            upstream_api: upstream_api.to_owned(),
            field: "reasoning",
        });
    }
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

fn min_known_limit(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn reasoning_rank(reasoning: ReasoningSupport) -> u8 {
    match reasoning {
        ReasoningSupport::Unsupported => 0,
        ReasoningSupport::Unknown => 1,
        ReasoningSupport::Supported => 2,
    }
}

fn is_valid_parameter_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_valid_environment_variable(locator: &str) -> bool {
    let mut characters = locator.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

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
