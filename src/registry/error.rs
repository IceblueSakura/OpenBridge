//! 注册表定义校验和编译错误。

use thiserror::Error;

/// 编译期注册表定义不完整、引用不一致或尝试越权时返回的错误。
#[derive(Debug, Error)]
pub enum RegistryError {
    /// registry 版本为空。
    #[error("registry version must not be blank")]
    BlankVersion,
    /// credential pool id 为空。
    #[error("credential pool id must not be blank")]
    BlankCredentialPoolId,
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
    /// pool 选择了 provider 不支持的 credential 类型。
    #[error("credential pool '{credential_pool}' uses a kind unsupported by its provider")]
    UnsupportedCredentialPoolKind {
        /// 配置不兼容的 pool id。
        credential_pool: String,
    },
    /// target 与引用 pool 的 Provider 不一致。
    #[error(
        "upstream target '{upstream_target}' and credential pool '{credential_pool}' use different providers"
    )]
    CredentialPoolProviderMismatch {
        /// 配置不兼容的 target id。
        upstream_target: String,
        /// 被错误引用的 pool id。
        credential_pool: String,
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
