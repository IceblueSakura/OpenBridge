//! Managed-key policy at the source boundary, without constructing a Gateway.
use super::*;
use crate::credential::{CredentialManager, Secret};
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id).unwrap();
        let id: String = id.iter().map(|b| format!("{b:02x}")).collect();
        Self(std::env::temp_dir().join(format!("openbridge-key-source-{id}")))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn managed_keys_enforce_domain_kind_and_epoch_without_network() {
    let dir = Directory::new();
    let manager = CredentialManager::new(&dir.0, vec![]).unwrap();
    let first = manager
        .add_api_key("alpha", "one", Secret::new("synthetic-old".into()).unwrap())
        .unwrap();
    let mut credentials = Credentials::new();
    let id = CredentialBindingId::new("alpha-key").unwrap();
    credentials.insert_managed_key(id.clone(), manager.bind_api_key("alpha", "one").unwrap());
    let source = &credentials.get(&id).unwrap().sources[0];
    assert!(source.check(CredentialKind::ApiKey, "alpha").is_ok());
    assert!(source.check(CredentialKind::ApiKey, "beta").is_err());
    assert!(
        source
            .check(CredentialKind::OAuth("alpha"), "alpha")
            .is_err()
    );
    let provenance = source.provenance();
    assert!(!provenance.contains("synthetic-old"));
    let (snapshot, grant) = source.resolve().unwrap();
    assert!(grant.is_none());
    manager
        .replace_api_key(
            "alpha",
            "one",
            first.revision,
            Secret::new("synthetic-new".into()).unwrap(),
        )
        .unwrap();
    let error = source.resolve().err().unwrap();
    assert_eq!(error.status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error.code, "credential_unavailable");
    assert_eq!(snapshot.expose(), "synthetic-old");
    let fresh = Source::ManagedKey(manager.bind_api_key("alpha", "one").unwrap());
    assert_ne!(fresh.provenance(), provenance);
    assert_eq!(fresh.resolve().unwrap().0.expose(), "synthetic-new");
}
