//! Embeddings interface DTO and request-time accessors.

use super::*;

/// Encoding contract exposed by one Embeddings execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingEncodingCapabilities {
    pub(super) default: EmbeddingEncoding,
    pub(super) allowed: Option<Vec<EmbeddingEncoding>>,
}

/// Dimension contract exposed by one Embeddings execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingDimensionCapabilities {
    pub(super) default: u32,
    pub(super) allowed: Option<EmbeddingDimensionDomain>,
}

/// Request limits exposed by one Embeddings execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingLimits {
    pub(super) max_inputs: u32,
    pub(super) max_tokens_per_input: Option<u32>,
    pub(super) max_total_tokens: Option<u32>,
    pub(super) locally_counted_input_forms: Vec<EmbeddingInputForm>,
}

/// Unique fixed capability contract for the Embeddings Create operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingInterfaceCapabilities {
    pub(super) input_forms: Vec<EmbeddingInputForm>,
    pub(super) encoding: EmbeddingEncodingCapabilities,
    pub(super) dimensions: EmbeddingDimensionCapabilities,
    pub(super) limits: EmbeddingLimits,
    pub(super) supported_parameters: Vec<String>,
}

impl EmbeddingInterfaceCapabilities {
    /// Returns whether this interface accepts the analyzed input form.
    pub(crate) fn supports_input_form(&self, input_form: EmbeddingInputForm) -> bool {
        self.input_forms.contains(&input_form)
    }

    /// Resolves an omitted or explicit encoding without adding a local conversion.
    pub(crate) fn resolve_encoding(
        &self,
        requested: Option<EmbeddingEncoding>,
    ) -> Option<EmbeddingEncoding> {
        match requested {
            None => Some(self.encoding.default),
            Some(requested)
                if self
                    .encoding
                    .allowed
                    .as_ref()
                    .is_some_and(|allowed| allowed.contains(&requested)) =>
            {
                Some(requested)
            }
            Some(_) => None,
        }
    }

    /// Resolves an omitted or explicit positive dimension against the fixed domain.
    pub(crate) fn resolve_dimensions(&self, requested: Option<u32>) -> Option<u32> {
        match requested {
            None => Some(self.dimensions.default),
            Some(requested)
                if self
                    .dimensions
                    .allowed
                    .is_some_and(|allowed| allowed.contains(requested)) =>
            {
                Some(requested)
            }
            Some(_) => None,
        }
    }

    /// Returns whether this interface exposes an optional top-level request parameter.
    pub(crate) fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|supported| supported == parameter)
    }

    /// Returns the maximum number of input items accepted by one request.
    pub(crate) const fn max_inputs(&self) -> u32 {
        self.limits.max_inputs
    }

    /// Returns the optional maximum token count for one locally countable input.
    pub(crate) const fn max_tokens_per_input(&self) -> Option<u32> {
        self.limits.max_tokens_per_input
    }

    /// Returns the optional maximum total token count for locally countable inputs.
    pub(crate) const fn max_total_tokens(&self) -> Option<u32> {
        self.limits.max_total_tokens
    }

    /// Returns whether this input form's token counts are enforced before egress.
    pub(crate) fn counts_tokens_locally(&self, input_form: EmbeddingInputForm) -> bool {
        self.limits
            .locally_counted_input_forms
            .contains(&input_form)
    }
}
