//! Single descriptor for a Provider's static contract and adapter.
//!
//! The descriptor aggregates compiled metadata only; it does not register targets, Routes, or
//! Public Models and does not read credentials.

use super::{ProviderAdapter, ProviderContract, ProviderKind};

/// Binds a Provider's static contract and closed adapter.
#[derive(Clone, Copy)]
pub struct ProviderDefinition {
    contract: &'static ProviderContract,
    adapter: ProviderAdapter,
}

impl ProviderDefinition {
    /// Creates a static descriptor owned by the concrete Provider module.
    pub(crate) const fn new(contract: &'static ProviderContract, adapter: ProviderAdapter) -> Self {
        Self { contract, adapter }
    }

    /// Returns the Provider kind represented by the descriptor.
    pub fn kind(&self) -> ProviderKind {
        self.contract.kind()
    }

    /// Returns the Provider's static capabilities and configuration ceiling.
    pub fn contract(&self) -> &'static ProviderContract {
        self.contract
    }

    /// Returns the Provider's closed request and response adapter.
    pub fn adapter(&self) -> ProviderAdapter {
        self.adapter
    }
}
