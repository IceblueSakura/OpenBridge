//! 编译期 Real Model、Upstream Target、Native Offering、Public Model 与 Serving Route 注册表。
//!
//! 模型事实由 `src/models/*` 构造，target/offering/route 由 `src/providers/*` 构造。
//! 启动时 builder 对完整定义执行引用、能力和安全边界校验，成功后生成请求路径只读的
//! `RegistrySnapshot`。

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use thiserror::Error;
use url::Url;

use crate::{
    config::{BootstrapPolicy, RuntimeLimits, UpstreamPolicy},
    core::{CapabilitySet, Protocol, ProtocolCapabilities, ResponsesCapabilities},
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
pub struct RealModelDefinition {
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
pub struct ModelConstraints {
    /// offering 可进一步收紧的上下文长度。
    pub context_length: ModelContextLength,
    /// offering 可进一步收紧的 reasoning 状态。
    pub reasoning: Option<ReasoningSupport>,
    /// offering 禁用但不能新增的参数名。
    pub disabled_parameters: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialDefinition {
    /// 注册表中的 credential id。
    pub id: String,
    /// adapter 支持的 credential 类型。
    pub kind: CredentialKind,
    /// 运行时读取 secret 的环境变量名。
    pub environment_variable: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOfferingCapabilities {
    ChatCompletions(ProtocolCapabilities),
    Responses(ResponsesCapabilities),
}

impl NativeOfferingCapabilities {
    pub const fn protocol(self) -> Protocol {
        match self {
            Self::ChatCompletions(_) => Protocol::ChatCompletions,
            Self::Responses(_) => Protocol::Responses,
        }
    }

    pub const fn protocol_capabilities(self) -> ProtocolCapabilities {
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

    const fn is_subset_of(self, upper: CapabilitySet) -> bool {
        match self {
            Self::ChatCompletions(capabilities) => {
                capabilities.is_subset_of(upper.chat_completions)
            }
            Self::Responses(capabilities) => capabilities.is_subset_of(upper.responses),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTransport {
    HttpJsonSse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatePolicy {
    Stateless,
    ProviderBound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOfferingDefinition {
    /// target 内稳定的 offering id。
    pub id: String,
    /// offering 原生提供的协议。
    pub protocol: Protocol,
    /// 发往上游的真实模型 id。
    pub upstream_model: String,
    /// provider 允许的 endpoint profile。
    pub endpoint_profile: String,
    /// 当前原生 transport profile。
    pub transport: NativeTransport,
    /// 对 Real Model 事实的 offering 级收窄。
    pub model_constraints: ModelConstraints,
    /// 单协议能力证据。
    pub capabilities: NativeOfferingCapabilities,
    /// continuation/state 所有权策略。
    pub state_policy: StatePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamTargetDefinition {
    /// 注册表中的 target id。
    pub id: String,
    /// 编译期 Provider Family。
    pub provider: ProviderKind,
    /// 引用的 Real Model id。
    pub real_model: String,
    /// 经过校验的 HTTPS endpoint base。
    pub base_url: String,
    /// target 使用的 credential binding。
    pub credential: CredentialDefinition,
    /// 可选的明确共享 quota scope。
    pub quota_scope: Option<String>,
    /// 可选的故障/cooldown 域。
    pub fault_domain: Option<String>,
    /// 单次上游请求超时时间。
    pub request_timeout: Duration,
    /// 是否允许新的无状态请求选择该 target。
    pub enabled: bool,
    /// target 原生提供的协议级供应。
    pub offerings: Vec<NativeOfferingDefinition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServingRouteMode {
    Native,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServingRouteDefinition {
    pub id: String,
    pub upstream_target: String,
    pub offering: String,
    pub downstream_protocol: Protocol,
    pub mode: ServingRouteMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicModelDefinition {
    /// 对下游公开的稳定 model name。
    pub name: String,
    /// 按优先级排列的完整 Serving Route id。
    pub serving_routes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryDefinition {
    /// 用于报告和审计的注册表版本。
    pub version: String,
    /// 完整模型定义集合。
    pub real_models: Vec<RealModelDefinition>,
    /// 完整 Upstream Target 定义集合。
    pub upstream_targets: Vec<UpstreamTargetDefinition>,
    /// 完整 Serving Route 定义集合。
    pub serving_routes: Vec<ServingRouteDefinition>,
    /// 完整 Public Model 定义集合。
    pub public_models: Vec<PublicModelDefinition>,
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
    #[error("upstream target '{upstream_target}' must contain at least one offering")]
    EmptyUpstreamTarget { upstream_target: String },
    #[error("upstream target '{upstream_target}' contains duplicate offering '{offering}'")]
    DuplicateOffering {
        upstream_target: String,
        offering: String,
    },
    #[error(
        "offering '{offering}' on upstream target '{upstream_target}' uses unsupported endpoint profile '{profile}'"
    )]
    UnsupportedEndpointProfile {
        upstream_target: String,
        offering: String,
        profile: String,
    },
    #[error(
        "offering '{offering}' on upstream target '{upstream_target}' upstream model must not be blank"
    )]
    BlankUpstreamModel {
        upstream_target: String,
        offering: String,
    },
    #[error(
        "offering '{offering}' on upstream target '{upstream_target}' capability type does not match protocol"
    )]
    OfferingProtocolMismatch {
        upstream_target: String,
        offering: String,
    },
    #[error("model '{model}' field '{field}' must not be blank")]
    BlankModelField { model: String, field: &'static str },
    #[error("model '{model}' context length '{limit}' must be greater than zero")]
    InvalidModelContextLength { model: String, limit: &'static str },
    #[error("model '{model}' declares invalid supported parameter '{parameter}'")]
    InvalidSupportedParameter { model: String, parameter: String },
    #[error("model '{model}' declares supported parameter '{parameter}' more than once")]
    DuplicateSupportedParameter { model: String, parameter: String },
    #[error("model '{model}' has inconsistent reasoning metadata: {detail}")]
    InconsistentReasoningMetadata { model: String, detail: &'static str },
    #[error("offering '{offering}' model constraint '{field}' must be greater than zero")]
    InvalidOfferingModelConstraint {
        offering: String,
        field: &'static str,
    },
    #[error("offering '{offering}' model constraint '{field}' exceeds the model limit")]
    OfferingModelConstraintExceedsModelLimit {
        offering: String,
        field: &'static str,
    },
    #[error("offering '{offering}' model constraint '{field}' widens the model metadata")]
    OfferingModelConstraintWidensModelMetadata {
        offering: String,
        field: &'static str,
    },
    #[error("offering '{offering}' model constraint disables undeclared parameter '{parameter}'")]
    OfferingModelConstraintDisablesUndeclaredParameter { offering: String, parameter: String },
    #[error("offering '{offering}' model constraints are inconsistent: {detail}")]
    InconsistentOfferingModelConstraints {
        offering: String,
        detail: &'static str,
    },
    #[error(
        "offering '{offering}' on upstream target '{upstream_target}' enables capabilities unsupported by its adapter"
    )]
    CapabilityElevation {
        upstream_target: String,
        offering: String,
    },
    #[error("native serving route '{route}' protocol does not match its offering")]
    NativeRouteProtocolMismatch { route: String },
    #[error("public model '{public_model}' contains duplicate serving route '{route}'")]
    DuplicatePublicModelRoute { public_model: String, route: String },
    #[error("public model '{public_model}' must contain at least one serving route")]
    EmptyPublicModel { public_model: String },
}

/// 启动后供请求路径读取的模型元数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelMetadata {
    id: String,
    name: String,
    description: Option<String>,
    context_length: ModelContextLength,
    supported_parameters: Vec<String>,
    reasoning: ReasoningSupport,
    reasoning_levels: Vec<ReasoningLevel>,
}

impl ModelMetadata {
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
pub struct RegistrySnapshot {
    version: RegistryVersion,
    bootstrap: BootstrapPolicy,
    real_models: BTreeMap<String, ModelMetadata>,
    upstream_targets: BTreeMap<String, ResolvedUpstreamTarget>,
    serving_routes: BTreeMap<String, ResolvedServingRoute>,
    public_models: BTreeMap<String, ResolvedPublicModel>,
}

impl RegistrySnapshot {
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
    pub fn upstream_policy(&self) -> &UpstreamPolicy {
        self.bootstrap.upstream_policy()
    }

    /// 按内部模型 id 查询模型元数据。
    pub fn real_model(&self, id: &str) -> Option<&ModelMetadata> {
        self.real_models.get(id)
    }

    /// 按内部 target id 查询解析结果。
    pub fn upstream_target(&self, id: &str) -> Option<&ResolvedUpstreamTarget> {
        self.upstream_targets.get(id)
    }

    /// 枚举所有内部 target id。
    pub fn upstream_target_ids(&self) -> impl Iterator<Item = &str> {
        self.upstream_targets.keys().map(String::as_str)
    }

    pub fn serving_route(&self, id: &str) -> Option<&ResolvedServingRoute> {
        self.serving_routes.get(id)
    }

    pub fn public_model(&self, name: &str) -> Option<&ResolvedPublicModel> {
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
pub struct ResolvedCredential {
    id: String,
    kind: CredentialKind,
    secret_reference: SecretReference,
}

impl ResolvedCredential {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub fn secret_reference(&self) -> &SecretReference {
        &self.secret_reference
    }
}

#[derive(Debug)]
pub struct ResolvedUpstreamTarget {
    kind: ProviderKind,
    credential: ResolvedCredential,
    real_model_id: String,
    endpoint_base: Url,
    quota_scope: Option<String>,
    fault_domain: Option<String>,
    request_timeout: Duration,
    enabled: bool,
    offerings: BTreeMap<String, ResolvedNativeOffering>,
}

impl ResolvedUpstreamTarget {
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn credential(&self) -> &ResolvedCredential {
        &self.credential
    }

    pub fn real_model_id(&self) -> &str {
        &self.real_model_id
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

    pub fn offering(&self, id: &str) -> Option<&ResolvedNativeOffering> {
        self.offerings.get(id)
    }

    pub fn offering_for_protocol(&self, protocol: Protocol) -> Option<&ResolvedNativeOffering> {
        self.offerings
            .values()
            .find(|offering| offering.protocol() == protocol)
    }

    pub fn offerings(&self) -> impl Iterator<Item = (&str, &ResolvedNativeOffering)> {
        self.offerings
            .iter()
            .map(|(id, offering)| (id.as_str(), offering))
    }
}

#[derive(Debug)]
pub struct SecretReference {
    locator: String,
}

impl SecretReference {
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
pub struct ResolvedNativeOffering {
    protocol: Protocol,
    model: ModelMetadata,
    upstream_model: String,
    endpoint_profile: String,
    transport: NativeTransport,
    capabilities: NativeOfferingCapabilities,
    state_policy: StatePolicy,
}

impl ResolvedNativeOffering {
    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn model(&self) -> &ModelMetadata {
        &self.model
    }

    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub fn endpoint_profile(&self) -> &str {
        &self.endpoint_profile
    }

    pub fn transport(&self) -> NativeTransport {
        self.transport
    }

    pub fn capabilities(&self) -> NativeOfferingCapabilities {
        self.capabilities
    }

    pub fn state_policy(&self) -> StatePolicy {
        self.state_policy
    }
}

#[derive(Debug)]
pub struct ResolvedServingRoute {
    upstream_target: String,
    offering: String,
    downstream_protocol: Protocol,
    mode: ServingRouteMode,
}

impl ResolvedServingRoute {
    pub fn upstream_target(&self) -> &str {
        &self.upstream_target
    }

    pub fn offering(&self) -> &str {
        &self.offering
    }

    pub fn downstream_protocol(&self) -> Protocol {
        self.downstream_protocol
    }

    pub fn mode(&self) -> ServingRouteMode {
        self.mode
    }
}

#[derive(Debug)]
pub struct ResolvedPublicModel {
    serving_routes: Vec<String>,
}

impl ResolvedPublicModel {
    pub fn serving_routes(&self) -> &[String] {
        &self.serving_routes
    }
}

pub fn build_registry(
    bootstrap: BootstrapPolicy,
    definition: RegistryDefinition,
) -> Result<RegistrySnapshot, RegistryError> {
    if definition.version.trim().is_empty() {
        return Err(RegistryError::BlankVersion);
    }

    let mut real_models = BTreeMap::new();
    for model in definition.real_models {
        validate_model_metadata(&model)?;
        let id = model.id.clone();
        let resolved = ModelMetadata {
            id: id.clone(),
            name: model.name,
            description: model.description,
            context_length: model.context_length,
            supported_parameters: model.supported_parameters,
            reasoning: model.reasoning,
            reasoning_levels: model.reasoning_levels,
        };
        if real_models.insert(id.clone(), resolved).is_some() {
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
        let model = real_models
            .get(&target.real_model)
            .cloned()
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "upstream target",
                id: target.id.clone(),
                target: "real model",
                reference: target.real_model.clone(),
            })?;
        if target.request_timeout.is_zero() {
            return Err(RegistryError::InvalidRequestTimeout {
                upstream_target: target.id,
            });
        }
        if target.offerings.is_empty() {
            return Err(RegistryError::EmptyUpstreamTarget {
                upstream_target: target.id,
            });
        }
        let endpoint_base = normalize_endpoint_base(&target.base_url).ok_or_else(|| {
            RegistryError::InvalidBaseUrl {
                upstream_target: target.id.clone(),
            }
        })?;
        let mut offerings = BTreeMap::new();
        for offering in target.offerings {
            if offering.protocol != offering.capabilities.protocol() {
                return Err(RegistryError::OfferingProtocolMismatch {
                    upstream_target: target.id,
                    offering: offering.id,
                });
            }
            if offering.upstream_model.trim().is_empty() {
                return Err(RegistryError::BlankUpstreamModel {
                    upstream_target: target.id,
                    offering: offering.id,
                });
            }
            if !target
                .provider
                .accepts_endpoint_profile(&offering.endpoint_profile)
            {
                return Err(RegistryError::UnsupportedEndpointProfile {
                    upstream_target: target.id,
                    offering: offering.id,
                    profile: offering.endpoint_profile,
                });
            }
            if !offering
                .capabilities
                .is_subset_of(target.provider.capabilities())
            {
                return Err(RegistryError::CapabilityElevation {
                    upstream_target: target.id,
                    offering: offering.id,
                });
            }
            let offering_key = format!("{}/{}", target.id, offering.id);
            let effective_model =
                apply_model_constraints(model.clone(), &offering_key, offering.model_constraints)?;
            let resolved = ResolvedNativeOffering {
                protocol: offering.protocol,
                model: effective_model,
                upstream_model: offering.upstream_model,
                endpoint_profile: offering.endpoint_profile,
                transport: offering.transport,
                capabilities: offering.capabilities,
                state_policy: offering.state_policy,
            };
            if offerings.insert(offering.id.clone(), resolved).is_some() {
                return Err(RegistryError::DuplicateOffering {
                    upstream_target: target.id,
                    offering: offering.id,
                });
            }
        }
        let resolved = ResolvedUpstreamTarget {
            kind: target.provider,
            credential: ResolvedCredential {
                id: target.credential.id,
                kind: target.credential.kind,
                secret_reference: SecretReference {
                    locator: target.credential.environment_variable,
                },
            },
            real_model_id: target.real_model,
            endpoint_base,
            quota_scope: target.quota_scope,
            fault_domain: target.fault_domain,
            request_timeout: target.request_timeout,
            enabled: target.enabled,
            offerings,
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

    let mut serving_routes = BTreeMap::new();
    for route in definition.serving_routes {
        let target = upstream_targets
            .get(&route.upstream_target)
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "serving route",
                id: route.id.clone(),
                target: "upstream target",
                reference: route.upstream_target.clone(),
            })?;
        let offering =
            target
                .offering(&route.offering)
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "serving route",
                    id: route.id.clone(),
                    target: "native offering",
                    reference: format!("{}/{}", route.upstream_target, route.offering),
                })?;
        if route.mode == ServingRouteMode::Native
            && route.downstream_protocol != offering.protocol()
        {
            return Err(RegistryError::NativeRouteProtocolMismatch { route: route.id });
        }
        let resolved = ResolvedServingRoute {
            upstream_target: route.upstream_target,
            offering: route.offering,
            downstream_protocol: route.downstream_protocol,
            mode: route.mode,
        };
        if serving_routes.insert(route.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "serving route",
                id: route.id,
            });
        }
    }

    let mut public_models = BTreeMap::new();
    for public_model in definition.public_models {
        if public_model.serving_routes.is_empty() {
            return Err(RegistryError::EmptyPublicModel {
                public_model: public_model.name,
            });
        }
        let mut seen = BTreeSet::new();
        for route in &public_model.serving_routes {
            if !seen.insert(route) {
                return Err(RegistryError::DuplicatePublicModelRoute {
                    public_model: public_model.name,
                    route: route.clone(),
                });
            }
            if !serving_routes.contains_key(route) {
                return Err(RegistryError::UnknownReference {
                    entity: "public model",
                    id: public_model.name,
                    target: "serving route",
                    reference: route.clone(),
                });
            }
        }
        if public_models
            .insert(
                public_model.name.clone(),
                ResolvedPublicModel {
                    serving_routes: public_model.serving_routes,
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

    Ok(RegistrySnapshot {
        version: RegistryVersion(definition.version),
        bootstrap,
        real_models,
        upstream_targets,
        serving_routes,
        public_models,
    })
}

fn validate_model_metadata(model: &RealModelDefinition) -> Result<(), RegistryError> {
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
    validate_reasoning_metadata(&model.id, &model.supported_parameters, model.reasoning).and_then(
        |()| validate_reasoning_levels(&model.id, model.reasoning, &model.reasoning_levels),
    )
}

fn validate_reasoning_levels(
    model: &str,
    reasoning: ReasoningSupport,
    levels: &[ReasoningLevel],
) -> Result<(), RegistryError> {
    if reasoning != ReasoningSupport::Supported && !levels.is_empty() {
        return Err(RegistryError::InconsistentReasoningMetadata {
            model: model.to_owned(),
            detail: "reasoning levels require reasoning = supported",
        });
    }
    let mut seen = BTreeSet::new();
    if levels.iter().any(|level| !seen.insert(*level)) {
        return Err(RegistryError::InconsistentReasoningMetadata {
            model: model.to_owned(),
            detail: "reasoning levels must not contain duplicates",
        });
    }
    Ok(())
}

fn validate_reasoning_metadata(
    model: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => Err(RegistryError::InconsistentReasoningMetadata {
            model: model.to_owned(),
            detail: "reasoning = supported requires supported_parameters to include reasoning",
        }),
        (ReasoningSupport::Unsupported, true) => {
            Err(RegistryError::InconsistentReasoningMetadata {
                model: model.to_owned(),
                detail: "reasoning = unsupported conflicts with supported_parameters",
            })
        }
        _ => Ok(()),
    }
}

fn apply_model_constraints(
    model: ModelMetadata,
    offering: &str,
    constraints: ModelConstraints,
) -> Result<ModelMetadata, RegistryError> {
    validate_constraint_limit(
        offering,
        "context_length.input",
        model.context_length.input_tokens(),
        constraints.context_length.input_tokens(),
    )?;
    validate_constraint_limit(
        offering,
        "context_length.output",
        model.context_length.output_tokens(),
        constraints.context_length.output_tokens(),
    )?;
    let reasoning = constraints.reasoning.unwrap_or(model.reasoning);
    if reasoning_rank(reasoning) > reasoning_rank(model.reasoning) {
        return Err(RegistryError::OfferingModelConstraintWidensModelMetadata {
            offering: offering.to_owned(),
            field: "reasoning",
        });
    }
    let disabled = constraints
        .disabled_parameters
        .iter()
        .collect::<BTreeSet<_>>();
    for parameter in &disabled {
        if !model.supported_parameters.contains(parameter) {
            return Err(
                RegistryError::OfferingModelConstraintDisablesUndeclaredParameter {
                    offering: offering.to_owned(),
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
    validate_effective_reasoning_metadata(offering, &supported_parameters, reasoning)?;
    Ok(ModelMetadata {
        id: model.id,
        name: model.name,
        description: model.description,
        context_length: ModelContextLength::new(
            min_known_limit(
                model.context_length.input_tokens(),
                constraints.context_length.input_tokens(),
            ),
            min_known_limit(
                model.context_length.output_tokens(),
                constraints.context_length.output_tokens(),
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

fn validate_constraint_limit(
    offering: &str,
    field: &'static str,
    model_limit: Option<u32>,
    constraint_limit: Option<u32>,
) -> Result<(), RegistryError> {
    let Some(constraint_limit) = constraint_limit else {
        return Ok(());
    };
    if constraint_limit == 0 {
        return Err(RegistryError::InvalidOfferingModelConstraint {
            offering: offering.to_owned(),
            field,
        });
    }
    if model_limit.is_some_and(|model_limit| constraint_limit > model_limit) {
        return Err(RegistryError::OfferingModelConstraintExceedsModelLimit {
            offering: offering.to_owned(),
            field,
        });
    }
    Ok(())
}

fn validate_effective_reasoning_metadata(
    offering: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => {
            Err(RegistryError::InconsistentOfferingModelConstraints {
                offering: offering.to_owned(),
                detail: "reasoning = supported requires the effective parameter set to include reasoning",
            })
        }
        (ReasoningSupport::Unsupported, true) => {
            Err(RegistryError::InconsistentOfferingModelConstraints {
                offering: offering.to_owned(),
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
