//! 编译期 Provider、Model、Deployment 与 Alias 注册表。
//!
//! 注册项由 `src/providers/*` 中的 Rust 代码显式构造。启动时 builder 对完整定义执行
//! 引用、能力和安全边界校验，成功后生成请求路径只读的 `RegistrySnapshot`。

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use thiserror::Error;
use url::Url;

use crate::{
    config::{BootstrapPolicy, RuntimeLimits, UpstreamPolicy},
    core::CapabilitySet,
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
pub struct ModelDefinition {
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
    /// deployment 可进一步收紧的上下文长度。
    pub context_length: ModelContextLength,
    /// deployment 可进一步收紧的 reasoning 状态。
    pub reasoning: Option<ReasoningSupport>,
    /// deployment 禁用但不能新增的参数名。
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDefinition {
    /// 注册表中的 provider id。
    pub id: String,
    /// 编译期 provider kind。
    pub kind: ProviderKind,
    /// 该 provider 使用的 credential binding。
    pub credential: CredentialDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentDefinition {
    /// 注册表中的 deployment id。
    pub id: String,
    /// 引用的 provider id。
    pub provider: String,
    /// 引用的模型 id。
    pub model: String,
    /// 发往上游的真实模型 id。
    pub upstream_model: String,
    /// provider 允许的 endpoint profile。
    pub endpoint_profile: String,
    /// 经过校验的 HTTPS endpoint base。
    pub base_url: String,
    /// 单次上游请求超时时间。
    pub request_timeout: Duration,
    /// 对模型目录能力的 deployment 级收窄。
    pub model_constraints: ModelConstraints,
    /// 对 provider 能力上界的 deployment 级收窄。
    pub capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasDefinition {
    /// 对下游公开的 model alias。
    pub name: String,
    /// 按优先级排列的 deployment id。
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryDefinition {
    /// 用于报告和审计的注册表版本。
    pub version: String,
    /// 完整模型定义集合。
    pub models: Vec<ModelDefinition>,
    /// 完整 provider 定义集合。
    pub providers: Vec<ProviderDefinition>,
    /// 完整 deployment 定义集合。
    pub deployments: Vec<DeploymentDefinition>,
    /// 完整 public alias 集合。
    pub aliases: Vec<AliasDefinition>,
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
    #[error("provider '{provider}' uses an invalid credential environment variable")]
    InvalidCredentialLocator { provider: String },
    #[error("provider '{provider}' uses a credential kind unsupported by its adapter")]
    UnsupportedCredentialKind { provider: String },
    #[error("deployment '{deployment}' uses an invalid base URL")]
    InvalidBaseUrl { deployment: String },
    #[error("deployment '{deployment}' uses unsupported endpoint profile '{profile}'")]
    UnsupportedEndpointProfile { deployment: String, profile: String },
    #[error("deployment '{deployment}' request timeout must be greater than zero")]
    InvalidRequestTimeout { deployment: String },
    #[error("deployment '{deployment}' upstream model must not be blank")]
    BlankUpstreamModel { deployment: String },
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
    #[error("deployment '{deployment}' model constraint '{field}' must be greater than zero")]
    InvalidDeploymentModelConstraint {
        deployment: String,
        field: &'static str,
    },
    #[error("deployment '{deployment}' model constraint '{field}' exceeds the model limit")]
    DeploymentModelConstraintExceedsModelLimit {
        deployment: String,
        field: &'static str,
    },
    #[error("deployment '{deployment}' model constraint '{field}' widens the model metadata")]
    DeploymentModelConstraintWidensModelMetadata {
        deployment: String,
        field: &'static str,
    },
    #[error(
        "deployment '{deployment}' model constraint disables undeclared parameter '{parameter}'"
    )]
    DeploymentModelConstraintDisablesUndeclaredParameter {
        deployment: String,
        parameter: String,
    },
    #[error("deployment '{deployment}' model constraints are inconsistent: {detail}")]
    InconsistentDeploymentModelConstraints {
        deployment: String,
        detail: &'static str,
    },
    #[error("deployment '{deployment}' enables capabilities unsupported by its adapter")]
    CapabilityElevation { deployment: String },
    #[error("alias '{alias}' contains duplicate deployment candidate '{candidate}'")]
    DuplicateAliasCandidate { alias: String, candidate: String },
    #[error("alias '{alias}' must contain at least one deployment candidate")]
    EmptyAlias { alias: String },
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
    models: BTreeMap<String, ModelMetadata>,
    providers: BTreeMap<String, ResolvedProvider>,
    deployments: BTreeMap<String, ResolvedDeployment>,
    aliases: BTreeMap<String, ResolvedAlias>,
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
    pub fn model(&self, id: &str) -> Option<&ModelMetadata> {
        self.models.get(id)
    }

    /// 按内部 provider id 查询解析结果。
    pub fn provider(&self, id: &str) -> Option<&ResolvedProvider> {
        self.providers.get(id)
    }

    /// 按内部 deployment id 查询解析结果。
    pub fn deployment(&self, id: &str) -> Option<&ResolvedDeployment> {
        self.deployments.get(id)
    }

    /// 枚举所有内部 deployment id。
    pub fn deployment_ids(&self) -> impl Iterator<Item = &str> {
        self.deployments.keys().map(String::as_str)
    }

    /// 按 public alias 查询路由候选。
    pub fn alias(&self, name: &str) -> Option<&ResolvedAlias> {
        self.aliases.get(name)
    }

    /// 枚举下游 `/v1/models` 可公开的 alias。
    pub fn public_aliases(&self) -> impl Iterator<Item = &str> {
        self.aliases.keys().map(String::as_str)
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
pub struct ResolvedProvider {
    kind: ProviderKind,
    credential: ResolvedCredential,
}

impl ResolvedProvider {
    /// 返回 provider kind。
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// 返回已解析的 credential binding。
    pub fn credential(&self) -> &ResolvedCredential {
        &self.credential
    }
}

#[derive(Debug)]
pub struct ResolvedCredential {
    id: String,
    kind: CredentialKind,
    secret_reference: SecretReference,
}

impl ResolvedCredential {
    /// 返回 credential id。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回 credential kind。
    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    /// 返回不含 secret 的 locator 引用。
    pub fn secret_reference(&self) -> &SecretReference {
        &self.secret_reference
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
pub struct ResolvedDeployment {
    provider_id: String,
    model: ModelMetadata,
    upstream_model: String,
    endpoint_profile: String,
    endpoint_base: Url,
    request_timeout: Duration,
    capabilities: CapabilitySet,
}

impl ResolvedDeployment {
    /// 返回所属 provider id。
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// 返回 deployment 生效后的模型元数据。
    pub fn model(&self) -> &ModelMetadata {
        &self.model
    }

    /// 返回发往上游的模型 id。
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// 返回 endpoint profile。
    pub fn endpoint_profile(&self) -> &str {
        &self.endpoint_profile
    }

    /// 返回经过注册表校验的 endpoint base。
    pub fn endpoint_base(&self) -> &Url {
        &self.endpoint_base
    }

    /// 返回单次请求超时时间。
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// 返回 deployment 的生效能力集合。
    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

#[derive(Debug)]
pub struct ResolvedAlias {
    candidates: Vec<String>,
}

impl ResolvedAlias {
    /// 返回按优先级排列的 deployment 候选。
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }
}

pub fn build_registry(
    bootstrap: BootstrapPolicy,
    definition: RegistryDefinition,
) -> Result<RegistrySnapshot, RegistryError> {
    if definition.version.trim().is_empty() {
        return Err(RegistryError::BlankVersion);
    }

    // 先解析模型与 provider，后续 deployment/alias 才能进行引用校验。
    let mut models = BTreeMap::new();
    for model in definition.models {
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
        if models.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "model",
                id,
            });
        }
    }

    let mut providers = BTreeMap::new();
    let mut credential_ids = BTreeSet::new();
    for provider in definition.providers {
        if !provider
            .kind
            .accepts_credential_kind(provider.credential.kind)
        {
            return Err(RegistryError::UnsupportedCredentialKind {
                provider: provider.id,
            });
        }
        if !is_valid_environment_variable(&provider.credential.environment_variable) {
            return Err(RegistryError::InvalidCredentialLocator {
                provider: provider.id,
            });
        }
        if !credential_ids.insert(provider.credential.id.clone()) {
            return Err(RegistryError::DuplicateId {
                entity: "credential",
                id: provider.credential.id,
            });
        }
        let resolved = ResolvedProvider {
            kind: provider.kind,
            credential: ResolvedCredential {
                id: provider.credential.id,
                kind: provider.credential.kind,
                secret_reference: SecretReference {
                    locator: provider.credential.environment_variable,
                },
            },
        };
        if providers.insert(provider.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "provider",
                id: provider.id,
            });
        }
    }

    // deployment 同时收紧模型元数据和 provider 能力，禁止配置越权放大能力。
    let mut deployments = BTreeMap::new();
    for deployment in definition.deployments {
        let provider =
            providers
                .get(&deployment.provider)
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "deployment",
                    id: deployment.id.clone(),
                    target: "provider",
                    reference: deployment.provider.clone(),
                })?;
        let model = models.get(&deployment.model).cloned().ok_or_else(|| {
            RegistryError::UnknownReference {
                entity: "deployment",
                id: deployment.id.clone(),
                target: "model",
                reference: deployment.model.clone(),
            }
        })?;
        let model = apply_model_constraints(model, &deployment.id, deployment.model_constraints)?;
        if deployment.upstream_model.trim().is_empty() {
            return Err(RegistryError::BlankUpstreamModel {
                deployment: deployment.id,
            });
        }
        if deployment.request_timeout.is_zero() {
            return Err(RegistryError::InvalidRequestTimeout {
                deployment: deployment.id,
            });
        }
        if !provider
            .kind
            .accepts_endpoint_profile(&deployment.endpoint_profile)
        {
            return Err(RegistryError::UnsupportedEndpointProfile {
                deployment: deployment.id,
                profile: deployment.endpoint_profile,
            });
        }
        if !deployment
            .capabilities
            .is_subset_of(provider.kind.capabilities())
        {
            return Err(RegistryError::CapabilityElevation {
                deployment: deployment.id,
            });
        }
        let endpoint_base = normalize_endpoint_base(&deployment.base_url).ok_or_else(|| {
            RegistryError::InvalidBaseUrl {
                deployment: deployment.id.clone(),
            }
        })?;
        let resolved = ResolvedDeployment {
            provider_id: deployment.provider,
            model,
            upstream_model: deployment.upstream_model,
            endpoint_profile: deployment.endpoint_profile,
            endpoint_base,
            request_timeout: deployment.request_timeout,
            capabilities: deployment.capabilities,
        };
        if deployments
            .insert(deployment.id.clone(), resolved)
            .is_some()
        {
            return Err(RegistryError::DuplicateId {
                entity: "deployment",
                id: deployment.id,
            });
        }
    }

    // 最后建立 public alias，确保每个候选都指向已完成校验的 deployment。
    let mut aliases = BTreeMap::new();
    for alias in definition.aliases {
        if alias.candidates.is_empty() {
            return Err(RegistryError::EmptyAlias { alias: alias.name });
        }
        let mut seen = BTreeSet::new();
        for candidate in &alias.candidates {
            if !seen.insert(candidate) {
                return Err(RegistryError::DuplicateAliasCandidate {
                    alias: alias.name,
                    candidate: candidate.clone(),
                });
            }
            if !deployments.contains_key(candidate) {
                return Err(RegistryError::UnknownReference {
                    entity: "alias",
                    id: alias.name,
                    target: "deployment",
                    reference: candidate.clone(),
                });
            }
        }
        if aliases
            .insert(
                alias.name.clone(),
                ResolvedAlias {
                    candidates: alias.candidates,
                },
            )
            .is_some()
        {
            return Err(RegistryError::DuplicateId {
                entity: "alias",
                id: alias.name,
            });
        }
    }

    Ok(RegistrySnapshot {
        version: RegistryVersion(definition.version),
        bootstrap,
        models,
        providers,
        deployments,
        aliases,
    })
}

fn validate_model_metadata(model: &ModelDefinition) -> Result<(), RegistryError> {
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
    deployment: &str,
    constraints: ModelConstraints,
) -> Result<ModelMetadata, RegistryError> {
    validate_constraint_limit(
        deployment,
        "context_length.input",
        model.context_length.input_tokens(),
        constraints.context_length.input_tokens(),
    )?;
    validate_constraint_limit(
        deployment,
        "context_length.output",
        model.context_length.output_tokens(),
        constraints.context_length.output_tokens(),
    )?;
    let reasoning = constraints.reasoning.unwrap_or(model.reasoning);
    if reasoning_rank(reasoning) > reasoning_rank(model.reasoning) {
        return Err(
            RegistryError::DeploymentModelConstraintWidensModelMetadata {
                deployment: deployment.to_owned(),
                field: "reasoning",
            },
        );
    }
    let disabled = constraints
        .disabled_parameters
        .iter()
        .collect::<BTreeSet<_>>();
    for parameter in &disabled {
        if !model.supported_parameters.contains(parameter) {
            return Err(
                RegistryError::DeploymentModelConstraintDisablesUndeclaredParameter {
                    deployment: deployment.to_owned(),
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
    validate_effective_reasoning_metadata(deployment, &supported_parameters, reasoning)?;
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
    deployment: &str,
    field: &'static str,
    model_limit: Option<u32>,
    constraint_limit: Option<u32>,
) -> Result<(), RegistryError> {
    let Some(constraint_limit) = constraint_limit else {
        return Ok(());
    };
    if constraint_limit == 0 {
        return Err(RegistryError::InvalidDeploymentModelConstraint {
            deployment: deployment.to_owned(),
            field,
        });
    }
    if model_limit.is_some_and(|model_limit| constraint_limit > model_limit) {
        return Err(RegistryError::DeploymentModelConstraintExceedsModelLimit {
            deployment: deployment.to_owned(),
            field,
        });
    }
    Ok(())
}

fn validate_effective_reasoning_metadata(
    deployment: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => {
            Err(RegistryError::InconsistentDeploymentModelConstraints {
                deployment: deployment.to_owned(),
                detail: "reasoning = supported requires the effective parameter set to include reasoning",
            })
        }
        (ReasoningSupport::Unsupported, true) => {
            Err(RegistryError::InconsistentDeploymentModelConstraints {
                deployment: deployment.to_owned(),
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
