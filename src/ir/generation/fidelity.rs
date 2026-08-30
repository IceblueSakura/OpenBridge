//! Fidelity accounting for pure Generation IR transformations.
//!
//! This module records semantic changes explicitly and applies the small, closed loss policy. It
//! does not perform decoding, encoding, routing, Registry access, or tool execution.

use std::borrow::Borrow;
use std::fmt;

use thiserror::Error;

/// Validation failure for one bounded fidelity provenance identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FidelityIdentityError {
    /// The identity is empty.
    #[error("fidelity identity must not be empty")]
    Empty,
    /// The identity exceeds its caller-supplied byte bound.
    #[error("fidelity identity exceeds the {max_bytes}-byte limit")]
    TooLarge {
        /// Maximum accepted UTF-8 bytes.
        max_bytes: usize,
    },
}

fn bounded_identity(
    value: impl Into<String>,
    max_bytes: usize,
) -> Result<String, FidelityIdentityError> {
    let value = value.into();
    if value.is_empty() {
        return Err(FidelityIdentityError::Empty);
    }
    if value.len() > max_bytes {
        return Err(FidelityIdentityError::TooLarge { max_bytes });
    }
    Ok(value)
}

/// A stable location in the canonical semantic value being transformed.
///
/// Paths are intentionally opaque to this layer: their syntax belongs to the value owner that
/// creates them. The empty path denotes the transformed value as a whole.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct SemanticPath(String);

impl SemanticPath {
    /// Creates a path without normalizing or interpreting its spelling.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the path for a whole-value change.
    pub fn root() -> Self {
        Self::default()
    }

    /// Returns whether this is the whole-value path.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the path as its stable textual representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Appends a field component using the conventional dotted spelling.
    pub fn field(&self, name: impl AsRef<str>) -> Self {
        let name = name.as_ref();
        if self.is_root() {
            Self::new(name)
        } else {
            Self::new(format!("{}.{}", self.0, name))
        }
    }

    /// Appends an index component using the conventional bracketed spelling.
    pub fn index(&self, index: usize) -> Self {
        Self::new(format!("{}[{}]", self.0, index))
    }
}

impl Borrow<str> for SemanticPath {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for SemanticPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for SemanticPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SemanticPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for SemanticPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The semantic disposition of a successful transformation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ChangeKind {
    /// A semantically equivalent representation was selected.
    Normalized,
    /// A value required by the target representation was synthesized.
    Synthesized,
    /// Provider-private opaque state was retained without reinterpretation.
    OpaquePreserved,
    /// A semantic operation was carried out by an explicitly authorized emulator.
    Emulated,
    /// Meaningful semantic information was removed or weakened.
    Lossy,
}

/// Why one semantic change was made.
///
/// The variants describe the small set of changes that are meaningful to the R1 kernel. Wire
/// codecs may use the associated constants below when a more domain-specific spelling is useful;
/// they still produce one of this closed enum's stable reasons.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ChangeReason {
    /// An inactive field or equivalent default was omitted.
    InactiveFieldOmitted,
    /// A protocol representation was normalized without changing meaning.
    ProtocolNormalized,
    /// A target envelope or required value was synthesized.
    TargetValueSynthesized,
    /// A missing canonical identity was generated.
    IdentitySynthesized,
    /// Opaque Provider state was preserved as opaque state.
    OpaqueStatePreserved,
    /// A trusted Gateway tool plan injected a declaration.
    ToolPlanInjection,
    /// A trusted Gateway tool plan stripped a declaration.
    ToolPlanStripping,
    /// A trusted Gateway executor emulated a semantic operation.
    GatewayEmulation,
    /// A meaningful semantic value was omitted.
    SemanticOmission,
}

/// Stable identity of a trusted compiled tool plan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ToolPlanId(String);

impl ToolPlanId {
    /// Creates a non-empty plan identity within the supplied UTF-8 byte bound.
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, FidelityIdentityError> {
        bounded_identity(value, max_bytes).map(Self)
    }

    /// Returns the plan identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ToolPlanId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for ToolPlanId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ToolPlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identity of one directive within a trusted tool plan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ToolDirectiveId(String);

impl ToolDirectiveId {
    /// Creates a non-empty directive identity within the supplied UTF-8 byte bound.
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, FidelityIdentityError> {
        bounded_identity(value, max_bytes).map(Self)
    }

    /// Returns the directive identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ToolDirectiveId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for ToolDirectiveId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ToolDirectiveId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Provenance authorizing a semantic change.
///
/// `None` means that the change is governed only by the selected [`LossPolicy`]. A tool
/// directive authorization is provenance, not a general switch that permits unrelated losses.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct ChangeAuthorization(AuthorizationKind);

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
enum AuthorizationKind {
    /// No narrow authorization exists.
    #[default]
    None,
    /// The change was produced by this exact trusted plan directive.
    #[cfg_attr(not(test), allow(dead_code))]
    ToolDirective {
        plan: ToolPlanId,
        directive: ToolDirectiveId,
        path: SemanticPath,
        reason: ChangeReason,
    },
}

impl ChangeAuthorization {
    /// Creates an authorization scoped to one exact change produced by a trusted directive.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn from_tool_directive(
        plan: ToolPlanId,
        directive: ToolDirectiveId,
        path: SemanticPath,
        reason: ChangeReason,
    ) -> Self {
        Self(AuthorizationKind::ToolDirective {
            plan,
            directive,
            path,
            reason,
        })
    }

    /// Returns the trusted plan/directive pair, when present.
    pub fn tool_directive(&self) -> Option<(&ToolPlanId, &ToolDirectiveId)> {
        match &self.0 {
            AuthorizationKind::None => None,
            AuthorizationKind::ToolDirective {
                plan, directive, ..
            } => Some((plan, directive)),
        }
    }

    fn authorizes(&self, path: &SemanticPath, reason: ChangeReason) -> bool {
        matches!(
            &self.0,
            AuthorizationKind::ToolDirective {
                path: authorized_path,
                reason: authorized_reason,
                ..
            } if authorized_path == path && *authorized_reason == reason
        )
    }

    /// Returns whether no narrow authorization is attached.
    pub fn is_none(&self) -> bool {
        matches!(self.0, AuthorizationKind::None)
    }
}

/// One explicitly observed semantic change.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SemanticChange {
    path: SemanticPath,
    kind: ChangeKind,
    reason: ChangeReason,
    authorization: ChangeAuthorization,
}

impl SemanticChange {
    /// Creates a change record from its semantic location, disposition, reason, and provenance.
    pub fn new(
        path: impl Into<SemanticPath>,
        kind: ChangeKind,
        reason: ChangeReason,
        authorization: ChangeAuthorization,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            reason,
            authorization,
        }
    }

    /// Returns the changed semantic path.
    pub fn path(&self) -> &SemanticPath {
        &self.path
    }

    /// Returns the change disposition.
    pub fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// Returns the reason for the change.
    pub fn reason(&self) -> ChangeReason {
        self.reason
    }

    /// Returns the authorization provenance.
    pub fn authorization(&self) -> &ChangeAuthorization {
        &self.authorization
    }

    /// Returns whether this record represents semantic loss.
    pub fn is_lossy(&self) -> bool {
        self.kind == ChangeKind::Lossy
    }
}

/// A value and the semantic changes observed while producing it.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Transform<T> {
    value: T,
    changes: Vec<SemanticChange>,
}

impl<T> Transform<T> {
    /// Creates a transformation result.
    pub fn new(value: T, changes: Vec<SemanticChange>) -> Self {
        Self { value, changes }
    }

    /// Creates an exact transformation with no recorded changes.
    pub fn exact(value: T) -> Self {
        Self::new(value, Vec::new())
    }

    /// Returns the transformed value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Returns the recorded changes in observation order.
    pub fn changes(&self) -> &[SemanticChange] {
        &self.changes
    }

    /// Consumes the result and returns the transformed value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consumes the result and returns its value and change report.
    pub fn into_parts(self) -> (T, Vec<SemanticChange>) {
        (self.value, self.changes)
    }

    /// Returns whether the transformation is exact.
    pub fn is_exact(&self) -> bool {
        self.changes.is_empty()
    }

    /// Maps the value while preserving the semantic change report.
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Transform<U> {
        Transform::new(map(self.value), self.changes)
    }

    /// Appends one observed change while preserving report order.
    pub fn with_change(mut self, change: SemanticChange) -> Self {
        self.changes.push(change);
        self
    }
}

/// Whether unscoped semantic loss is permitted.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LossPolicy {
    /// Reject every lossy change without a tool-directive authorization.
    #[default]
    Reject,
    /// Permit unscoped lossy changes.
    Allow,
}

/// Failure while checking a fidelity report against its loss policy.
#[derive(Clone, Debug, Eq, Error, Hash, PartialEq)]
pub enum FidelityError {
    /// A lossy change was not authorized by either a tool directive or an Allow policy.
    #[error("lossy semantic change at '{path}' is rejected ({reason:?})")]
    LossRejected {
        /// Semantic location of the rejected change.
        path: SemanticPath,
        /// Why the semantic value would be lost.
        reason: ChangeReason,
    },
}

/// Returns whether a transformation contains no semantic changes.
pub fn exact<T>(transform: &Transform<T>) -> bool {
    transform.is_exact()
}

/// Enforces the global loss policy over a transformation report.
///
/// A valid tool-directive authorization is intentionally narrower than [`LossPolicy::Allow`]: it
/// permits only the explicitly authorized lossy record, while unrelated unscoped losses remain
/// subject to the global policy.
pub fn enforce_loss_policy<T>(
    transform: &Transform<T>,
    policy: LossPolicy,
) -> Result<(), FidelityError> {
    for change in transform.changes() {
        if !change.is_lossy() {
            continue;
        }

        if change
            .authorization()
            .authorizes(change.path(), change.reason())
            || policy == LossPolicy::Allow
        {
            continue;
        }

        return Err(FidelityError::LossRejected {
            path: change.path().clone(),
            reason: change.reason(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lossy_change(authorization: ChangeAuthorization) -> SemanticChange {
        SemanticChange::new(
            SemanticPath::new("input[0].content"),
            ChangeKind::Lossy,
            ChangeReason::SemanticOmission,
            authorization,
        )
    }

    #[test]
    fn exact_reports_only_empty_change_lists() {
        assert!(exact(&Transform::exact(42_u8)));
        assert!(!exact(&Transform::new(
            42_u8,
            vec![SemanticChange::new(
                SemanticPath::root(),
                ChangeKind::Normalized,
                ChangeReason::InactiveFieldOmitted,
                ChangeAuthorization::default(),
            )],
        )));
    }

    #[test]
    fn reject_policy_allows_authorized_loss_but_not_unscoped_loss() {
        let authorized = Transform::new(
            (),
            vec![lossy_change(ChangeAuthorization::from_tool_directive(
                ToolPlanId::new("plan-1", 64).expect("plan ID must fit"),
                ToolDirectiveId::new("directive-1", 64).expect("directive ID must fit"),
                SemanticPath::new("input[0].content"),
                ChangeReason::SemanticOmission,
            ))],
        );
        assert!(enforce_loss_policy(&authorized, LossPolicy::Reject).is_ok());

        let mismatched = Transform::new(
            (),
            vec![lossy_change(ChangeAuthorization::from_tool_directive(
                ToolPlanId::new("plan-1", 64).expect("plan ID must fit"),
                ToolDirectiveId::new("directive-1", 64).expect("directive ID must fit"),
                SemanticPath::new("tools[0]"),
                ChangeReason::SemanticOmission,
            ))],
        );
        assert!(matches!(
            enforce_loss_policy(&mismatched, LossPolicy::Reject),
            Err(FidelityError::LossRejected { .. })
        ));

        let rejected = Transform::new((), vec![lossy_change(ChangeAuthorization::default())]);
        assert!(matches!(
            enforce_loss_policy(&rejected, LossPolicy::Reject),
            Err(FidelityError::LossRejected { .. })
        ));
        assert!(enforce_loss_policy(&rejected, LossPolicy::Allow).is_ok());
    }

    #[test]
    fn semantic_path_builders_preserve_ordered_components() {
        let path = SemanticPath::root().field("input").index(2).field("text");
        assert_eq!(path.as_str(), "input[2].text");
    }
}
