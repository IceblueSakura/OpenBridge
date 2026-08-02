//! 为集成测试提供不含真实凭证的 bootstrap、registry 和 credential fixture。

#![allow(dead_code)]

mod fixtures;
pub mod process_replay;

#[allow(unused_imports)]
pub use fixtures::{
    BOOTSTRAP, bootstrap, capabilities, definition, prepare, registry, users_and_credential_pool,
    users_and_credentials,
};
