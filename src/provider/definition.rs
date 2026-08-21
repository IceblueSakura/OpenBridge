//! Single descriptor for a Provider's static contract and adapter.
//!
//! The descriptor aggregates compiled metadata only; it does not register targets, Routes, or
//! Public Models and does not read credentials.

use crate::core::{ApiCapabilities, OperationKind};

use super::{
    CredentialKind, ProviderAdapter, ProviderContract, ProviderKind, ProviderOperationAdapter,
};

/// Binds a Provider's static contract and closed adapter.
#[derive(Clone, Copy)]
pub struct ProviderDefinition {
    contract: ProviderContract,
    adapter: ProviderAdapter,
}

impl ProviderDefinition {
    /// Creates a static descriptor and derives the contract identity from its closed adapter.
    pub(crate) const fn new(
        capabilities: ApiCapabilities,
        credential_kinds: &'static [CredentialKind],
        adapter: ProviderAdapter,
    ) -> Self {
        let contract = ProviderContract::new(adapter.kind(), capabilities, credential_kinds);
        Self { contract, adapter }
    }

    /// Returns the Provider kind represented by the descriptor.
    pub fn kind(&self) -> ProviderKind {
        self.contract.kind()
    }

    /// Returns the Provider's static capabilities and configuration ceiling.
    pub fn contract(&self) -> &ProviderContract {
        &self.contract
    }

    /// Returns the Provider's closed request and response adapter.
    pub fn adapter(&self) -> ProviderAdapter {
        self.adapter
    }

    /// Selects one typed operation adapter from the Provider's static capability surface.
    pub fn operation_adapter(&self, operation: OperationKind) -> Option<ProviderOperationAdapter> {
        let capabilities = self.contract.capabilities().operation(operation)?;
        self.adapter.operation_adapter(operation, capabilities)
    }
}
