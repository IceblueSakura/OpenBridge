//! OpenBridge 编译进二进制的 Provider、Upstream Target 与路由目录。
//!
//! 新 Provider 必须在这里显式注册；不会通过配置、链接器或运行时插件自动出现。

pub mod meituan;
pub mod openai;

use crate::{
    config::BootstrapPolicy,
    core::Protocol,
    models,
    registry::{
        PublicModelDefinition, RegistryDefinition, RegistryError, RegistrySnapshot,
        ServingRouteDefinition, ServingRouteMode, build_registry,
    },
};

/// 当前内置 provider/model registry 的版本标识。
pub const REGISTRY_VERSION: &str = "dev-1";

/// 返回所有编译进二进制的 Real Model、Upstream Target、Serving Route 与 Public Model。
pub fn compiled_definition() -> RegistryDefinition {
    let openai = openai::definition();
    let meituan = meituan::definition();
    RegistryDefinition {
        version: REGISTRY_VERSION.to_owned(),
        real_models: models::compiled_definitions(),
        upstream_targets: [openai.upstream_targets, meituan.upstream_targets].concat(),
        serving_routes: vec![
            native_route(
                "code-primary-openai-chat",
                "openai-main",
                "chat",
                Protocol::ChatCompletions,
            ),
            native_route(
                "code-primary-openai-responses",
                "openai-main",
                "responses",
                Protocol::Responses,
            ),
            native_route(
                "longcat-2-meituan-chat",
                "meituan-longcat-2",
                "chat",
                Protocol::ChatCompletions,
            ),
            native_route(
                "longcat-2-meituan-responses",
                "meituan-longcat-2",
                "responses",
                Protocol::Responses,
            ),
        ],
        public_models: vec![
            PublicModelDefinition {
                name: "code-primary".to_owned(),
                serving_routes: vec![
                    "code-primary-openai-chat".to_owned(),
                    "code-primary-openai-responses".to_owned(),
                ],
            },
            PublicModelDefinition {
                name: "LongCat-2.0".to_owned(),
                serving_routes: vec![
                    "longcat-2-meituan-chat".to_owned(),
                    "longcat-2-meituan-responses".to_owned(),
                ],
            },
        ],
    }
}

fn native_route(
    id: &str,
    upstream_target: &str,
    offering: &str,
    downstream_protocol: Protocol,
) -> ServingRouteDefinition {
    ServingRouteDefinition {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        offering: offering.to_owned(),
        downstream_protocol,
        mode: ServingRouteMode::Native,
    }
}

/// 校验并构造内置 registry snapshot。
pub fn build_compiled_registry(
    bootstrap: BootstrapPolicy,
) -> Result<RegistrySnapshot, RegistryError> {
    build_registry(bootstrap, compiled_definition())
}
