//! 编译期 Provider 契约与闭合 adapter dispatch 的包入口。
//!
//! 路由配置只能选择已编译的 Provider；具体子模块分别拥有静态契约、请求响应分派和
//! 安全 header/SSE/error 数据契约。本文件仅声明模块并保持既有公共 API 路径。

mod adapter;
mod contracts;
mod kind;

pub use adapter::{AdapterError, PreparedUpstreamRequest, ProviderAdapter};
pub use contracts::{
    ClassifiedSseEvent, RetryHint, SafeHeaders, SensitiveHeaders, StatusClassification,
    StreamEventStatus, UpstreamErrorKind,
};
pub use kind::{CredentialKind, ProviderContract, ProviderKind};
