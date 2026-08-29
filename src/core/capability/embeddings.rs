//! Embeddings Create capability ceilings and closed-domain validation.
//!
//! Embeddings has no generation protocol, stream, reasoning, or Bridge semantics. Its input,
//! encoding, dimensions, and request limits are validated and narrowed as one independent domain.

/// Input shapes accepted by the Embeddings Create request.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInputForm {
    /// One non-empty string.
    String,
    /// A non-empty array of non-empty strings.
    StringArray,
    /// One non-empty token-ID array.
    TokenArray,
    /// A non-empty array of non-empty token-ID arrays.
    TokenArrayArray,
}

/// Embedding vector encodings guaranteed on the downstream wire.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingEncoding {
    /// A JSON array of floating-point components.
    #[default]
    Float,
    /// A standard Base64 string produced upstream or by an explicit fixed-interface translation.
    Base64,
}

/// Target/API-scoped translation between downstream and upstream embedding encodings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EmbeddingEncodingPolicy {
    /// Preserve the downstream encoding in both directions.
    #[default]
    Preserve,
    /// Request float vectors upstream and transcode them to Base64 only when requested downstream.
    Base64ViaFloat,
}

/// Explicit domain accepted by the Embeddings `dimensions` request field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EmbeddingDimensionDomain {
    /// A closed inclusive integer range.
    Range {
        /// Smallest accepted positive dimension.
        minimum: u32,
        /// Largest accepted positive dimension.
        maximum: u32,
    },
    /// A closed ordered set of accepted positive dimensions.
    Values {
        /// Accepted dimension values in ascending order.
        values: &'static [u32],
    },
}

impl EmbeddingDimensionDomain {
    /// Returns whether the domain contains one dimension.
    pub(crate) fn contains(self, value: u32) -> bool {
        match self {
            Self::Range { minimum, maximum } => (minimum..=maximum).contains(&value),
            Self::Values { values } => values.contains(&value),
        }
    }

    /// Returns whether every value in this domain is also accepted by the upper domain.
    fn is_subset_of(self, upper: Self) -> bool {
        match (self, upper) {
            (
                Self::Range { minimum, maximum },
                Self::Range {
                    minimum: upper_minimum,
                    maximum: upper_maximum,
                },
            ) => minimum >= upper_minimum && maximum <= upper_maximum,
            (Self::Values { values }, upper) => values.iter().all(|value| upper.contains(*value)),
            (Self::Range { minimum, maximum }, Self::Values { values }) => {
                let expected_len = maximum
                    .checked_sub(minimum)
                    .and_then(|width| width.checked_add(1))
                    .and_then(|length| usize::try_from(length).ok());
                expected_len == Some(values.len())
                    && values.first() == Some(&minimum)
                    && values.last() == Some(&maximum)
                    && values.windows(2).all(|pair| pair[1] == pair[0] + 1)
            }
        }
    }
}

/// Complete Upstream API capability profile for Embeddings Create.
///
/// The profile contains only fixed request and response guarantees. It does not contain Provider,
/// endpoint, credential, or Route identity and is projected into an owned public interface by the
/// registry compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingsCapabilities {
    /// Accepted non-empty input shapes in deterministic enum order.
    pub input_forms: &'static [EmbeddingInputForm],
    /// Encoding produced when `encoding_format` is omitted.
    pub default_encoding: EmbeddingEncoding,
    /// Encodings clients may request explicitly; `None` forbids the request field.
    pub allowed_encodings: Option<&'static [EmbeddingEncoding]>,
    /// Positive vector dimension produced when `dimensions` is omitted.
    pub default_dimensions: u32,
    /// Dimension domain clients may request explicitly; `None` forbids the request field.
    pub allowed_dimensions: Option<EmbeddingDimensionDomain>,
    /// Maximum number of input items accepted by one request.
    pub max_inputs: u32,
    /// Optional maximum token count for each input item.
    pub max_tokens_per_input: Option<u32>,
    /// Optional maximum total token count for one request.
    pub max_total_tokens: Option<u32>,
    /// Input forms whose token counts can be computed locally without a tokenizer.
    pub locally_counted_input_forms: &'static [EmbeddingInputForm],
    /// Optional top-level request parameters accepted by this Native API.
    pub supported_parameters: &'static [&'static str],
}

impl EmbeddingsCapabilities {
    /// Validates closed sets, defaults, domains, limits, and parameter ownership.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        // Validate non-empty deterministic input and explicit-encoding domains.
        if !is_strictly_sorted(self.input_forms) {
            return Err("input_forms must be non-empty, unique, and ordered");
        }
        if let Some(encodings) = self.allowed_encodings
            && (!is_strictly_sorted(encodings) || !encodings.contains(&self.default_encoding))
        {
            return Err(
                "allowed_encodings must be non-empty, unique, ordered, and contain the default",
            );
        }

        // Validate the positive default and any explicit dimension domain.
        if self.default_dimensions == 0 {
            return Err("default_dimensions must be greater than zero");
        }
        if let Some(domain) = self.allowed_dimensions {
            match domain {
                EmbeddingDimensionDomain::Range { minimum, maximum }
                    if minimum == 0 || minimum > maximum =>
                {
                    return Err("allowed dimension range must be positive and ordered");
                }
                EmbeddingDimensionDomain::Values { values }
                    if !is_strictly_sorted(values) || values.first() == Some(&0) =>
                {
                    return Err("allowed dimension values must be positive, unique, and ordered");
                }
                _ => {}
            }
            if !domain.contains(self.default_dimensions) {
                return Err("allowed dimensions must contain the default");
            }
        }

        // Validate positive request limits and their internal ordering.
        if self.max_inputs == 0
            || self.max_tokens_per_input == Some(0)
            || self.max_total_tokens == Some(0)
        {
            return Err("embedding limits must be greater than zero");
        }
        if self
            .max_tokens_per_input
            .is_some_and(|per_input| self.max_total_tokens.is_some_and(|total| per_input > total))
        {
            return Err("max_tokens_per_input must not exceed max_total_tokens");
        }

        // Restrict local counting to the ordered token-array subset of accepted input forms.
        if !is_sorted_unique_or_empty(self.locally_counted_input_forms)
            || self.locally_counted_input_forms.iter().any(|form| {
                !self.input_forms.contains(form)
                    || !matches!(
                        form,
                        EmbeddingInputForm::TokenArray | EmbeddingInputForm::TokenArrayArray
                    )
            })
        {
            return Err("locally counted forms must be an ordered accepted token-array subset");
        }

        // Keep the optional parameter set closed and consistent with explicit domains.
        if !is_sorted_unique_or_empty(self.supported_parameters)
            || self
                .supported_parameters
                .iter()
                .any(|parameter| !matches!(*parameter, "dimensions" | "encoding_format" | "user"))
        {
            return Err(
                "supported_parameters must be an ordered subset of the Embeddings allowlist",
            );
        }
        if self.supported_parameters.contains(&"encoding_format")
            != self.allowed_encodings.is_some()
            || self.supported_parameters.contains(&"dimensions")
                != self.allowed_dimensions.is_some()
        {
            return Err(
                "supported parameters must match the explicit encoding and dimension domains",
            );
        }
        Ok(())
    }

    /// Returns whether this API profile stays within a Provider capability ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        if self
            .input_forms
            .iter()
            .any(|form| !upper.input_forms.contains(form))
            || !encoding_supported_by(upper, self.default_encoding)
            || !dimension_supported_by(upper, self.default_dimensions)
            || !limit_is_subset(self.max_inputs, upper.max_inputs)
            || !optional_limit_is_subset(self.max_tokens_per_input, upper.max_tokens_per_input)
            || !optional_limit_is_subset(self.max_total_tokens, upper.max_total_tokens)
            || self
                .locally_counted_input_forms
                .iter()
                .any(|form| !upper.locally_counted_input_forms.contains(form))
            || self
                .supported_parameters
                .iter()
                .any(|parameter| !upper.supported_parameters.contains(parameter))
        {
            return false;
        }
        if self.allowed_encodings.is_some_and(|encodings| {
            upper.allowed_encodings.is_none_or(|upper_encodings| {
                encodings
                    .iter()
                    .any(|encoding| !upper_encodings.contains(encoding))
            })
        }) {
            return false;
        }
        match (self.allowed_dimensions, upper.allowed_dimensions) {
            (Some(domain), Some(upper_domain)) => domain.is_subset_of(upper_domain),
            (Some(_), None) => false,
            _ => true,
        }
    }
}

/// Returns whether one non-empty slice is strictly ordered and therefore duplicate-free.
fn is_strictly_sorted<T: Ord>(values: &[T]) -> bool {
    !values.is_empty() && values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns whether a slice is empty or strictly ordered and duplicate-free.
fn is_sorted_unique_or_empty<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns whether the Provider ceiling can produce or explicitly accept one encoding.
fn encoding_supported_by(upper: EmbeddingsCapabilities, value: EmbeddingEncoding) -> bool {
    upper.default_encoding == value
        || upper
            .allowed_encodings
            .is_some_and(|encodings| encodings.contains(&value))
}

/// Returns whether the Provider ceiling can produce or explicitly accept one dimension.
fn dimension_supported_by(upper: EmbeddingsCapabilities, value: u32) -> bool {
    upper.default_dimensions == value
        || upper
            .allowed_dimensions
            .is_some_and(|domain| domain.contains(value))
}

/// Returns whether a required positive limit is no wider than the Provider ceiling.
fn limit_is_subset(value: u32, upper: u32) -> bool {
    upper == 0 || value <= upper
}

/// Returns whether an optional limit is no wider than a known Provider ceiling.
fn optional_limit_is_subset(value: Option<u32>, upper: Option<u32>) -> bool {
    match (value, upper) {
        (Some(value), Some(upper)) => value <= upper,
        (None, Some(_)) => false,
        (_, None) => true,
    }
}
