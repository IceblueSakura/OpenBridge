//! 验证下游用户文件的 schema、启停状态、API Key 认证和错误脱敏行为。

use openbridge::identity::{UserConfiguration, UserRegistryError};

const USERS: &str = r#"
schema_version = 1

[[users]]
id = "alice"
name = "Alice"
api_key = "alice-downstream-api-key-00000001"
enabled = true

[[users]]
id = "disabled-user"
name = "Disabled User"
api_key = "disabled-user-api-key-0000000000"
enabled = false
"#;

#[test]
fn user_registry_authenticates_enabled_users_and_hides_keys() {
    let configuration = UserConfiguration::from_toml(USERS).unwrap();
    let (users, credentials) = configuration.into_parts();
    let credentials = credentials.build();

    let alice = users
        .authenticate(&credentials, "alice-downstream-api-key-00000001")
        .expect("enabled user must authenticate");
    assert_eq!(alice.id(), "alice");
    assert_eq!(alice.name(), "Alice");
    assert!(
        users
            .authenticate(&credentials, "wrong-api-key-value-000000000000")
            .is_none()
    );
    assert!(
        users
            .authenticate(&credentials, "disabled-user-api-key-0000000000")
            .is_none()
    );
    assert!(!format!("{users:?} {credentials:?}").contains("alice-downstream-api-key"));
}

#[test]
fn invalid_user_files_fail_before_runtime() {
    let duplicate_id = USERS.replace("id = \"disabled-user\"", "id = \"alice\"");
    assert_eq!(
        UserConfiguration::from_toml(&duplicate_id).unwrap_err(),
        UserRegistryError::DuplicateUserId {
            id: "alice".to_owned()
        }
    );

    let duplicate_key = USERS.replace(
        "disabled-user-api-key-0000000000",
        "alice-downstream-api-key-00000001",
    );
    assert_eq!(
        UserConfiguration::from_toml(&duplicate_key).unwrap_err(),
        UserRegistryError::DuplicateApiKey
    );

    let all_disabled = USERS.replace("enabled = true", "enabled = false");
    assert_eq!(
        UserConfiguration::from_toml(&all_disabled).unwrap_err(),
        UserRegistryError::NoEnabledUsers
    );
}
