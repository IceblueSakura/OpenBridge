//! 请求分析与 Route 规划的稳定错误类型。

use thiserror::Error;

/// 请求不能被安全地绑定到兼容 Route 时返回的规划错误。
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
    /// Public Model 没有可用的 route。
    #[error("configured model has no route candidate")]
    NoRoute,
    /// route 与请求协议不匹配。
    #[error("selected route does not support this protocol")]
    UnsupportedProtocol,
    /// route 不支持请求的 streaming 模式。
    #[error("selected route does not support streaming")]
    StreamingUnsupported,
    /// route 不支持请求声明的 capability。
    #[error("selected route does not support requested capabilities")]
    UnsupportedCapabilities,
    /// 请求的最大输出超过了生效上限。
    #[error("requested maximum output exceeds the configured model limit")]
    OutputLimitExceeded,
    /// 模型不支持请求的 reasoning。
    #[error("selected model does not support requested reasoning")]
    ReasoningUnsupported,
    /// 模型不支持请求的 reasoning level。
    #[error("selected model does not support the requested reasoning level")]
    ReasoningLevelUnsupported,
}
