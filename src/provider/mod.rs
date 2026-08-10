//! Package entry point for compile-time Provider contracts and closed adapter dispatch.
//!
//! Route configuration can select only compiled Providers. Submodules own static contracts,
//! request/response dispatch, and safe header/SSE/error data contracts. This file only declares
//! modules and preserves existing public API paths.

mod adapter;
mod contracts;
mod definition;
mod kind;

pub(crate) use adapter::ProviderRequestContext;
pub use adapter::{AdapterError, PreparedUpstreamRequest, ProviderAdapter};
pub use contracts::{
    ClassifiedSseEvent, RetryHint, SafeHeaders, SensitiveHeaders, StatusClassification,
    StreamEventStatus, UpstreamErrorKind,
};
pub(crate) use contracts::{ProviderRequestHeaders, StaticRequestHeader};
pub use definition::ProviderDefinition;
pub use kind::{CredentialKind, ProviderContract, ProviderKind};
