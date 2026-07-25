//! OpenBridge 编译进二进制的 Provider 与模型目录。
//!
//! 新 Provider 必须在这里显式注册；不会通过配置、链接器或运行时插件自动出现。

pub mod openai;

use crate::{
    config::BootstrapPolicy,
    registry::{
        AliasDefinition, RegistryDefinition, RegistryError, RegistrySnapshot, build_registry,
    },
};

pub const REGISTRY_VERSION: &str = "dev-1";

pub fn compiled_definition() -> RegistryDefinition {
    let openai = openai::definition();
    RegistryDefinition {
        version: REGISTRY_VERSION.to_owned(),
        models: openai.models,
        providers: vec![openai.provider],
        deployments: openai.deployments,
        aliases: vec![AliasDefinition {
            name: "code-primary".to_owned(),
            candidates: vec!["openai-main".to_owned()],
        }],
    }
}

pub fn build_compiled_registry(
    bootstrap: BootstrapPolicy,
) -> Result<RegistrySnapshot, RegistryError> {
    build_registry(bootstrap, compiled_definition())
}
