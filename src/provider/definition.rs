//! Provider 静态 contract 与 adapter 的单一描述符。
//!
//! 描述符只聚合已编译元数据，不注册 target、Route 或 Public Model，也不读取 credential。

use super::{ProviderAdapter, ProviderContract, ProviderKind};

/// 同时绑定一个 Provider 的静态 contract 与闭合 adapter。
#[derive(Clone, Copy)]
pub struct ProviderDefinition {
    contract: &'static ProviderContract,
    adapter: ProviderAdapter,
}

impl ProviderDefinition {
    /// 创建由具体 Provider 模块拥有的静态描述符。
    pub(crate) const fn new(contract: &'static ProviderContract, adapter: ProviderAdapter) -> Self {
        Self { contract, adapter }
    }

    /// 返回描述符对应的 Provider kind。
    pub fn kind(&self) -> ProviderKind {
        self.contract.kind()
    }

    /// 返回 Provider 的静态能力与配置上界。
    pub fn contract(&self) -> &'static ProviderContract {
        self.contract
    }

    /// 返回 Provider 的闭合请求与响应 adapter。
    pub fn adapter(&self) -> ProviderAdapter {
        self.adapter
    }
}
