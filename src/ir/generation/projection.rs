//! Requested semantic output projections independent of wire encoding.

use std::collections::BTreeSet;

use crate::core::ResponseInclude;

/// Additional response information requested by the downstream operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutputProjection {
    includes: BTreeSet<ResponseInclude>,
}

impl OutputProjection {
    /// Creates a deterministic projection set from accepted semantic include values.
    pub fn new(includes: impl IntoIterator<Item = ResponseInclude>) -> Self {
        Self {
            includes: includes.into_iter().collect(),
        }
    }

    /// Returns requested include values in deterministic enum order.
    pub fn includes(&self) -> &BTreeSet<ResponseInclude> {
        &self.includes
    }
}
