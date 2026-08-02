//! 内置 Model、Upstream Target、Route 与 Public Model 的编译目录装配。

mod public_models;
mod routes;

use crate::{
    config::BootstrapConfig,
    models,
    registry::{RegistryConfig, RegistryError, RuntimeRegistry, build_registry},
};

use super::{longcat, openai};

/// 当前内置 provider/model registry 的版本标识。
pub const REGISTRY_VERSION: &str = "dev-1";

/// 返回所有编译进二进制的 Model、Upstream Target、Route 与 Public Model。
pub fn compiled_config() -> RegistryConfig {
    RegistryConfig {
        version: REGISTRY_VERSION.to_owned(),
        models: models::compiled_configs(),
        upstream_targets: [openai::upstream_targets(), longcat::upstream_targets()].concat(),
        routes: routes::compiled_routes(),
        public_models: public_models::compiled_public_models(),
    }
}

/// 校验并构造内置 registry。
pub fn build_compiled_registry(
    bootstrap: BootstrapConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry(bootstrap, compiled_config())
}
