//! Provider origin, opaque replay state, cache state, and wire identity.

use super::{BoundedBytes, SemanticValidationError, leaf::bounded_string};

/// Stable namespace identifying the Provider family that owns opaque state.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderNamespace(String);

impl ProviderNamespace {
    /// Creates a non-empty bounded Provider namespace.
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        Ok(Self(bounded_string(
            value,
            "Provider namespace",
            max_bytes,
        )?))
    }

    /// Returns the namespace string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Trusted Provider identity attached to state that may be replayed or returned.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProviderOrigin {
    namespace: ProviderNamespace,
    value: String,
}

impl ProviderOrigin {
    /// Creates an origin from already validated namespace and identity values.
    pub fn new(
        namespace: ProviderNamespace,
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        Ok(Self {
            namespace,
            value: bounded_string(value, "Provider origin", max_bytes)?,
        })
    }

    /// Returns the owning Provider namespace.
    pub fn namespace(&self) -> &ProviderNamespace {
        &self.namespace
    }

    /// Returns the bounded Provider identity.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Closed category of Provider-owned opaque state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OpaqueKind {
    /// A Provider-issued continuation or response handle.
    Continuation,
    /// A Provider session identifier.
    Session,
    /// A Provider thought/reasoning signature.
    ThoughtSignature,
    /// Provider encrypted reasoning content.
    EncryptedContent,
    /// Other state accepted only by an explicit Provider extension profile.
    Extension,
}

/// Whether opaque state may cross the downstream response boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpaqueExposure {
    /// The owning protocol may return this state to the downstream client.
    Returnable,
    /// The state remains internal to the Gateway/Provider interaction.
    InternalOnly,
}

/// Opaque, origin-aware Provider state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpaqueState {
    namespace: ProviderNamespace,
    kind: OpaqueKind,
    payload: BoundedBytes,
    origin: Option<ProviderOrigin>,
    exposure: OpaqueExposure,
}

impl OpaqueState {
    /// Creates opaque state without interpreting its payload, while validating provenance.
    pub fn new(
        namespace: ProviderNamespace,
        kind: OpaqueKind,
        payload: BoundedBytes,
        origin: Option<ProviderOrigin>,
        exposure: OpaqueExposure,
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
            exposure,
        })
    }

    /// Returns the namespace that defines the payload semantics.
    pub fn namespace(&self) -> &ProviderNamespace {
        &self.namespace
    }

    /// Returns the closed opaque state category.
    pub const fn kind(&self) -> OpaqueKind {
        self.kind
    }

    /// Returns the uninterpreted bounded payload.
    pub fn payload(&self) -> &BoundedBytes {
        &self.payload
    }

    /// Returns the Provider origin, when this state is replay-affine.
    pub fn origin(&self) -> Option<&ProviderOrigin> {
        self.origin.as_ref()
    }

    /// Returns the downstream exposure policy.
    pub const fn exposure(&self) -> OpaqueExposure {
        self.exposure
    }
}

/// Bounded cache-key value retained independently from Provider wire naming.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheKey(String);

impl CacheKey {
    /// Creates a non-empty cache key within the supplied UTF-8 byte bound.
    pub fn new(
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SemanticValidationError> {
        Ok(Self(bounded_string(value, "cache key", max_bytes)?))
    }

    /// Returns the cache key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Portable cache retention request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheRetention {
    /// Provider-default in-memory retention.
    InMemory,
    /// Retain the cache entry for approximately 24 hours when supported.
    Hours24,
}

/// Portable cache directive; omission of either field remains observable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheDirective {
    key: Option<CacheKey>,
    retention: Option<CacheRetention>,
}

impl CacheDirective {
    /// Creates a cache directive from independently optional fields.
    pub const fn new(key: Option<CacheKey>, retention: Option<CacheRetention>) -> Self {
        Self { key, retention }
    }

    /// Returns the explicit cache key.
    pub fn key(&self) -> Option<&CacheKey> {
        self.key.as_ref()
    }

    /// Returns the explicit retention request.
    pub const fn retention(&self) -> Option<CacheRetention> {
        self.retention
    }
}

/// Opaque continuation restricted to the continuation semantic kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationState(OpaqueState);

impl ContinuationState {
    /// Creates continuation state and rejects session/reasoning/extension payloads.
    pub fn new(state: OpaqueState) -> Result<Self, SemanticValidationError> {
        if state.kind() != OpaqueKind::Continuation {
            return Err(SemanticValidationError::InvalidContinuationKind);
        }
        Ok(Self(state))
    }

    /// Returns the origin-aware opaque continuation.
    pub const fn state(&self) -> &OpaqueState {
        &self.0
    }
}

/// Request state kept separate from generation controls and Provider execution state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestState {
    continuation: Option<ContinuationState>,
    cache: Option<CacheDirective>,
    background: bool,
}

impl RequestState {
    /// Creates request state from optional continuation/cache state and a background flag.
    pub const fn new(
        continuation: Option<ContinuationState>,
        cache: Option<CacheDirective>,
        background: bool,
    ) -> Self {
        Self {
            continuation,
            cache,
            background,
        }
    }

    /// Returns the opaque continuation, when one is requested.
    pub fn continuation(&self) -> Option<&ContinuationState> {
        self.continuation.as_ref()
    }

    /// Returns the cache directive, when requested.
    pub fn cache(&self) -> Option<&CacheDirective> {
        self.cache.as_ref()
    }

    /// Returns whether background execution is requested.
    pub const fn background(&self) -> bool {
        self.background
    }
}

/// Provider wire identity kept separate from Gateway canonical identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WireIdentity {
    namespace: ProviderNamespace,
    value: String,
    origin: ProviderOrigin,
}

impl WireIdentity {
    /// Creates an identity from a namespace, bounded value, and owning origin.
    pub fn new(
        namespace: ProviderNamespace,
        value: impl Into<String>,
        max_bytes: usize,
        origin: ProviderOrigin,
    ) -> Result<Self, SemanticValidationError> {
        if origin.namespace() != &namespace {
            return Err(SemanticValidationError::OriginNamespaceMismatch);
        }
        Ok(Self {
            namespace,
            value: bounded_string(value, "wire identity", max_bytes)?,
            origin,
        })
    }

    /// Returns the Provider namespace.
    pub fn namespace(&self) -> &ProviderNamespace {
        &self.namespace
    }

    /// Returns the Provider wire value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the owning Provider origin.
    pub fn origin(&self) -> &ProviderOrigin {
        &self.origin
    }
}
