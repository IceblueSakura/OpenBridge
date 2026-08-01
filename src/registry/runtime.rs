//! 注册表编译后的不可变运行时实体。

use std::{collections::BTreeMap, time::Duration};

use url::Url;

use crate::{
    config::{BootstrapConfig, HttpClientConfig, RuntimeLimits},
    core::ApiProtocol,
    provider::{CredentialKind, ProviderKind},
};

use super::{
    ModelContextLength, ReasoningLevel, ReasoningSupport, RouteMode, StateAffinity, TransportKind,
    UpstreamApiCapabilities,
};

/// 启动后供请求路径读取的模型元数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInfo {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) context_length: ModelContextLength,
    pub(super) supported_parameters: Vec<String>,
    pub(super) reasoning: ReasoningSupport,
    pub(super) reasoning_levels: Vec<ReasoningLevel>,
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
    pub(super) version: RegistryVersion,
    pub(super) bootstrap: BootstrapConfig,
    pub(super) models: BTreeMap<String, ModelInfo>,
    pub(super) upstream_targets: BTreeMap<String, UpstreamTarget>,
    pub(super) routes: BTreeMap<String, Route>,
    pub(super) public_models: BTreeMap<String, PublicModel>,
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
pub struct RegistryVersion(pub(super) String);

impl RegistryVersion {
    /// 返回版本字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 已解析的 credential binding。
#[derive(Debug)]
pub struct CredentialBinding {
    pub(super) id: String,
    pub(super) kind: CredentialKind,
    pub(super) secret_reference: SecretLocator,
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
    pub(super) kind: ProviderKind,
    pub(super) credential: CredentialBinding,
    pub(super) model_id: String,
    pub(super) endpoint_base: Url,
    pub(super) quota_scope: Option<String>,
    pub(super) fault_domain: Option<String>,
    pub(super) request_timeout: Duration,
    pub(super) enabled: bool,
    pub(super) upstream_apis: BTreeMap<String, UpstreamApi>,
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
    pub(super) locator: String,
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
    pub(super) protocol: ApiProtocol,
    pub(super) model: ModelInfo,
    pub(super) upstream_model: String,
    pub(super) endpoint_profile: String,
    pub(super) transport: TransportKind,
    pub(super) capabilities: UpstreamApiCapabilities,
    pub(super) state_affinity: StateAffinity,
    pub(super) reasoning_level_mappings: BTreeMap<ReasoningLevel, String>,
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
    pub(super) upstream_target: String,
    pub(super) upstream_api: String,
    pub(super) downstream_protocol: ApiProtocol,
    pub(super) mode: RouteMode,
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
    pub(super) routes: Vec<String>,
}

impl PublicModel {
    /// 返回按优先级排列的 route id。
    pub fn routes(&self) -> &[String] {
        &self.routes
    }
}
