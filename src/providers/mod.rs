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

/// 当前内置 provider/model registry 的版本标识。
pub const REGISTRY_VERSION: &str = "dev-1";

/// 返回所有编译进二进制的 provider、model、deployment 和 alias 定义。
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

/// 校验并构造内置 registry snapshot。
pub fn build_compiled_registry(
    bootstrap: BootstrapPolicy,
) -> Result<RegistrySnapshot, RegistryError> {
    build_registry(bootstrap, compiled_definition())
}
