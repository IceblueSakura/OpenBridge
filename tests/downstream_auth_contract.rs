use openbridge::identity::{UserRegistry, UserRegistryError};

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
    let users = UserRegistry::from_toml(USERS).unwrap();

    let alice = users
        .authenticate("alice-downstream-api-key-00000001")
        .expect("enabled user must authenticate");
    assert_eq!(alice.id(), "alice");
    assert_eq!(alice.name(), "Alice");
    assert!(
        users
            .authenticate("wrong-api-key-value-000000000000")
            .is_none()
    );
    assert!(
        users
            .authenticate("disabled-user-api-key-0000000000")
            .is_none()
    );
    assert!(!format!("{users:?}").contains("alice-downstream-api-key"));
}

#[test]
fn invalid_user_files_fail_before_runtime() {
    let duplicate_id = USERS.replace("id = \"disabled-user\"", "id = \"alice\"");
    assert_eq!(
        UserRegistry::from_toml(&duplicate_id).unwrap_err(),
        UserRegistryError::DuplicateUserId {
            id: "alice".to_owned()
        }
    );

    let duplicate_key = USERS.replace(
        "disabled-user-api-key-0000000000",
        "alice-downstream-api-key-00000001",
    );
    assert_eq!(
        UserRegistry::from_toml(&duplicate_key).unwrap_err(),
        UserRegistryError::DuplicateApiKey
    );

    let all_disabled = USERS.replace("enabled = true", "enabled = false");
    assert_eq!(
        UserRegistry::from_toml(&all_disabled).unwrap_err(),
        UserRegistryError::NoEnabledUsers
    );
}
