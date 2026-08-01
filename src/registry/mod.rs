//! 编译期定义与请求路径只读注册表的包入口。
//!
//! 各子模块分别拥有静态定义、编译错误、运行时实体和启动期编译逻辑；本文件仅声明
//! 模块并保持既有公共 API 路径。

mod compiler;
mod definition;
mod error;
mod runtime;
mod validation;

pub use compiler::build_registry;
pub use definition::{
    CredentialConfig, ModelConfig, ModelContextLength, PublicModelConfig, ReasoningLevel,
    ReasoningLevelMapping, ReasoningSupport, RegistryConfig, RouteConfig, RouteMode, StateAffinity,
    TransportKind, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
    UpstreamTargetConfig,
};
pub use error::RegistryError;
pub use runtime::{
    CredentialBinding, ModelInfo, PublicModel, RegistryVersion, Route, RuntimeRegistry,
    SecretLocator, UpstreamApi, UpstreamTarget,
};
