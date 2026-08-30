//! Bounded Provider-private semantic extensions.

use serde_json::Value;

use crate::ir::generation::value::encoded_json_len;

use super::{
    BoundedBytes, ProviderNamespace, ProviderOrigin, SemanticValidationError, leaf::bounded_string,
};

/// Bounded opaque JSON object accepted only in Provider extensions.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedOpaqueJson(Value);

impl BoundedOpaqueJson {
    /// Creates an opaque JSON object after checking its shape and encoded bound.
    pub fn new(value: Value, max_bytes: usize) -> Result<Self, SemanticValidationError> {
        if !value.is_object() {
            return Err(SemanticValidationError::InvalidOpaqueJson);
        }
        if encoded_json_len(&value, max_bytes).is_none() {
            return Err(SemanticValidationError::OpaqueJsonTooLarge { max_bytes });
        }
        Ok(Self(value))
    }

    /// Returns the bounded JSON object.
    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

impl Eq for BoundedOpaqueJson {}

/// Opaque payload forms allowed for a Provider extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpaquePayload {
    /// Bounded JSON object payload.
    Json(BoundedOpaqueJson),
    /// Bounded binary payload.
    Bytes(BoundedBytes),
}

/// Closed extension kind owned by one Provider namespace.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExtensionKind(String);

impl ExtensionKind {
    /// Creates a bounded, non-empty extension kind label.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::ir::generation) fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        Ok(Self(bounded_string(value, "extension kind", max_bytes)?))
    }

    /// Returns the extension kind label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-private semantic extension that cannot be mistaken for portable IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExtension {
    namespace: ProviderNamespace,
    kind: ExtensionKind,
    payload: OpaquePayload,
    origin: Option<ProviderOrigin>,
}

impl ProviderExtension {
    /// Creates an origin-aware Provider extension.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::ir::generation) fn new(
        namespace: ProviderNamespace,
        kind: ExtensionKind,
        payload: OpaquePayload,
        origin: Option<ProviderOrigin>,
    ) -> Result<Self, SemanticValidationError> {
        if origin
            .as_ref()
            .is_some_and(|origin| origin.namespace() != &namespace)
        {
            return Err(SemanticValidationError::OriginNamespaceMismatch);
        }
        Ok(Self {
            namespace,
            kind,
            payload,
            origin,
        })
    }

    /// Returns the extension namespace.
    pub fn namespace(&self) -> &ProviderNamespace {
        &self.namespace
    }

    /// Returns the closed extension kind.
    pub fn kind(&self) -> &ExtensionKind {
        &self.kind
    }

    /// Returns the bounded opaque payload.
    pub fn payload(&self) -> &OpaquePayload {
        &self.payload
    }

    /// Returns the Provider origin, when the extension is origin-bound.
    pub fn origin(&self) -> Option<&ProviderOrigin> {
        self.origin.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn extension_constructor_rejects_mismatched_origin_namespace() {
        let namespace = ProviderNamespace::new("responses", 64).expect("namespace must fit");
        let kind = ExtensionKind::new("known-test-kind", 64).expect("kind must fit");
        let payload = OpaquePayload::Json(
            BoundedOpaqueJson::new(json!({"value": 1}), 128).expect("payload must fit"),
        );
        let origin = ProviderOrigin::new(
            ProviderNamespace::new("other", 64).expect("namespace must fit"),
            "target/api",
            128,
        )
        .expect("origin must fit");

        assert!(ProviderExtension::new(namespace, kind, payload, Some(origin)).is_err());
    }
}
