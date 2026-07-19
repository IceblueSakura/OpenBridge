use http::{HeaderMap, HeaderValue, header::AUTHORIZATION};
use openbridge::ingress::StaticBearerCredential;
use secrecy::SecretString;

#[test]
fn static_downstream_bearer_credential_fails_closed() {
    let credential = StaticBearerCredential::new(SecretString::from(
        "downstream-credential-test-value".to_owned(),
    ));

    let missing = HeaderMap::new();
    assert!(!credential.authenticate(&missing));

    for value in [
        "downstream-credential-test-value",
        "Basic downstream-credential-test-value",
        "Bearer wrong-value",
        "Bearer  downstream-credential-test-value",
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        assert!(!credential.authenticate(&headers));
    }

    let mut valid = HeaderMap::new();
    valid.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer downstream-credential-test-value"),
    );
    assert!(credential.authenticate(&valid));
    assert!(!format!("{credential:?}").contains("downstream-credential-test-value"));

    let empty = StaticBearerCredential::new(SecretString::from(String::new()));
    let mut empty_header = HeaderMap::new();
    empty_header.insert(AUTHORIZATION, HeaderValue::from_static("Bearer "));
    assert!(!empty.authenticate(&empty_header));
}
