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
    Unknown,
    Supported,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReasoningLevel {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl ReasoningLevel {
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
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

impl ModelContextLength {
    pub const fn new(input_tokens: Option<u32>, output_tokens: Option<u32>) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    pub const fn input_tokens(self) -> Option<u32> {
        self.input_tokens
    }

    pub const fn output_tokens(self) -> Option<u32> {
        self.output_tokens
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub context_length: ModelContextLength,
    pub supported_parameters: Vec<String>,
    pub reasoning: ReasoningSupport,
    pub reasoning_levels: Vec<ReasoningLevel>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelConstraints {
    pub context_length: ModelContextLength,
    pub reasoning: Option<ReasoningSupport>,
    pub disabled_parameters: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialDefinition {
    pub id: String,
    pub kind: CredentialKind,
    pub environment_variable: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDefinition {
    pub id: String,
    pub kind: ProviderKind,
    pub credential: CredentialDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentDefinition {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub upstream_model: String,
    pub endpoint_profile: String,
    pub base_url: String,
    pub request_timeout: Duration,
    pub model_constraints: ModelConstraints,
    pub capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasDefinition {
    pub name: String,
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryDefinition {
    pub version: String,
    pub models: Vec<ModelDefinition>,
    pub providers: Vec<ProviderDefinition>,
    pub deployments: Vec<DeploymentDefinition>,
    pub aliases: Vec<AliasDefinition>,
}

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
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub const fn context_length(&self) -> ModelContextLength {
        self.context_length
    }

    pub fn supported_parameters(&self) -> &[String] {
        &self.supported_parameters
    }

    pub const fn reasoning(&self) -> ReasoningSupport {
        self.reasoning
    }

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
    pub fn version(&self) -> &RegistryVersion {
        &self.version
    }

    pub fn listen(&self) -> std::net::SocketAddr {
        self.bootstrap.listen()
    }

    pub fn limits(&self) -> &RuntimeLimits {
        self.bootstrap.limits()
    }

    pub fn upstream_policy(&self) -> &UpstreamPolicy {
        self.bootstrap.upstream_policy()
    }

    pub fn model(&self, id: &str) -> Option<&ModelMetadata> {
        self.models.get(id)
    }

    pub fn provider(&self, id: &str) -> Option<&ResolvedProvider> {
        self.providers.get(id)
    }

    pub fn deployment(&self, id: &str) -> Option<&ResolvedDeployment> {
        self.deployments.get(id)
    }

    pub fn deployment_ids(&self) -> impl Iterator<Item = &str> {
        self.deployments.keys().map(String::as_str)
    }

    pub fn alias(&self, name: &str) -> Option<&ResolvedAlias> {
        self.aliases.get(name)
    }

    pub fn public_aliases(&self) -> impl Iterator<Item = &str> {
        self.aliases.keys().map(String::as_str)
    }
}

#[derive(Debug)]
pub struct RegistryVersion(String);

impl RegistryVersion {
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
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

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
pub struct SecretReference {
    locator: String,
}

impl SecretReference {
    pub fn scheme(&self) -> &'static str {
        "env"
    }

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
    pub fn provider_id(&self) -> &str {
        &self.provider_id
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

    pub fn endpoint_base(&self) -> &Url {
        &self.endpoint_base
    }

    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

#[derive(Debug)]
pub struct ResolvedAlias {
    candidates: Vec<String>,
}

impl ResolvedAlias {
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
