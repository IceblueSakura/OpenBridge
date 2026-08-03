//! 请求分析与 Route 规划的稳定错误类型。

use thiserror::Error;

/// 请求不能通过 Public Model 预检或绑定到配置 Route 时返回的规划错误。
#[derive(Debug, Error)]
pub enum RequestPlanningError {
    /// 请求 body 不是 JSON object。
    #[error("request body must be a JSON object")]
    InvalidJson,
    /// 请求缺少非空的 public model。
    #[error("request body must contain a non-empty model")]
    MissingModel,
    /// 请求的 public model 未在 registry 中注册。
    #[error("requested model is not configured")]
    UnknownModel,
    /// Public Model 没有静态可执行 Route。
    #[error("configured model has no executable route")]
    NoRoute,
    /// Public Model 没有请求协议对应的固定接口。
    #[error("selected model does not support this protocol")]
    UnsupportedProtocol,
    /// Public Model 的固定接口不支持 streaming。
    #[error("selected model does not support streaming")]
    StreamingUnsupported,
    /// Public Model 的固定接口不支持请求能力。
    #[error("selected model does not support requested capabilities")]
    UnsupportedCapabilities,
    /// 请求使用了已命名但尚未实现的预留 capability。
    #[error("requested capabilities are reserved but not implemented")]
    UnimplementedCapabilities,
    /// 请求的最大输出超过了生效上限。
    #[error("requested maximum output exceeds the configured model limit")]
    OutputLimitExceeded,
    /// 模型不支持请求的 reasoning。
    #[error("selected model does not support requested reasoning")]
    ReasoningUnsupported,
    /// 模型不支持请求的 reasoning level。
    #[error("selected model does not support the requested reasoning level")]
    ReasoningLevelUnsupported,
    /// 请求同时提供了冲突的 reasoning 配置来源或形状。
    #[error("request contains conflicting reasoning configuration")]
    InvalidReasoningConfiguration,
}
