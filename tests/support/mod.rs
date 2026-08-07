//! Provides bootstrap, registry, and credential fixtures without real credentials for integration tests.

#![allow(dead_code)]

mod fixtures;
pub mod metrics;
pub mod process_replay;

#[allow(unused_imports)]
pub use fixtures::{
    BOOTSTRAP, bootstrap, capabilities, definition, prepare, registry, users_and_credential_pool,
    users_and_credentials, users_and_oauth_credentials,
};
