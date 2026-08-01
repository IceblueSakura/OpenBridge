//! OpenBridge 编译进二进制的 Provider、Upstream Target 与路由目录。
//!
//! 新 Provider 必须在这里显式注册；不会通过配置、链接器或运行时插件自动出现。

pub mod longcat;
pub mod openai;

use crate::{
    config::BootstrapConfig,
    core::ApiProtocol,
    models,
    registry::{
        PublicModelConfig, RegistryConfig, RegistryError, RouteConfig, RouteMode, RuntimeRegistry,
        build_registry,
    },
};

/// 当前内置 provider/model registry 的版本标识。
pub const REGISTRY_VERSION: &str = "dev-1";

/// 返回所有编译进二进制的 Model、Upstream Target、Route 与 Public Model。
pub fn compiled_config() -> RegistryConfig {
    RegistryConfig {
        version: REGISTRY_VERSION.to_owned(),
        models: models::compiled_configs(),
        upstream_targets: [openai::upstream_targets(), longcat::upstream_targets()].concat(),
        routes: vec![
            native_route(
                "code-primary-openai-chat",
                "openai-main",
                "chat",
                ApiProtocol::ChatCompletions,
            ),
            native_route(
                "code-primary-openai-responses",
                "openai-main",
                "responses",
                ApiProtocol::Responses,
            ),
            native_route(
                "longcat-2-chat",
                "longcat-2",
                "chat",
                ApiProtocol::ChatCompletions,
            ),
            native_route(
                "longcat-2-responses",
                "longcat-2",
                "responses",
                ApiProtocol::Responses,
            ),
        ],
        public_models: vec![
            PublicModelConfig {
                name: "code-primary".to_owned(),
                routes: vec![
                    "code-primary-openai-chat".to_owned(),
                    "code-primary-openai-responses".to_owned(),
                ],
            },
            PublicModelConfig {
                name: "LongCat-2.0".to_owned(),
                routes: vec![
                    "longcat-2-chat".to_owned(),
                    "longcat-2-responses".to_owned(),
                ],
            },
        ],
    }
}

fn native_route(
    id: &str,
    upstream_target: &str,
    upstream_api: &str,
    downstream_protocol: ApiProtocol,
) -> RouteConfig {
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_api: upstream_api.to_owned(),
        downstream_protocol,
        mode: RouteMode::Native,
    }
}

/// 校验并构造内置 registry registry。
pub fn build_compiled_registry(
    bootstrap: BootstrapConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry(bootstrap, compiled_config())
}
