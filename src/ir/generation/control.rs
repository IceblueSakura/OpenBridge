//! Portable Generation controls with explicit absent-versus-value semantics.

use super::{ParallelToolCalls, SemanticValidationError, TextValue};

/// Finite floating-point control value preserving its exact IEEE-754 representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiniteF64(u64);

impl FiniteF64 {
    /// Creates a finite value and rejects NaN or infinity before it can enter Static IR.
    pub fn new(value: f64) -> Result<Self, SemanticValidationError> {
        if !value.is_finite() {
            return Err(SemanticValidationError::NonFiniteControl);
        }
        Ok(Self(value.to_bits()))
    }

    /// Returns the finite floating-point value.
    pub fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// Portable generation controls whose absence remains distinct from an explicit value.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenerationControls {
    max_output_tokens: Option<u64>,
    candidate_count: Option<u32>,
    temperature: Option<FiniteF64>,
    top_p: Option<FiniteF64>,
    top_k: Option<u64>,
    stop: Option<Vec<TextValue>>,
    seed: Option<i64>,
    frequency_penalty: Option<FiniteF64>,
    presence_penalty: Option<FiniteF64>,
    parallel_tool_calls: ParallelToolCalls,
}

impl GenerationControls {
    /// Creates count controls and rejects zero-valued limits.
    pub fn new(
        max_output_tokens: Option<u64>,
        candidate_count: Option<u32>,
    ) -> Result<Self, SemanticValidationError> {
        if max_output_tokens == Some(0) {
            return Err(SemanticValidationError::ZeroOutputLimit);
        }
        if candidate_count == Some(0) {
            return Err(SemanticValidationError::ZeroCandidateCount);
        }
        Ok(Self {
            max_output_tokens,
            candidate_count,
            ..Self::default()
        })
    }

    /// Replaces sampling controls after rejecting non-finite values.
    pub fn with_sampling(
        mut self,
        temperature: Option<f64>,
        top_p: Option<f64>,
        top_k: Option<u64>,
    ) -> Result<Self, SemanticValidationError> {
        self.temperature = temperature.map(FiniteF64::new).transpose()?;
        self.top_p = top_p.map(FiniteF64::new).transpose()?;
        self.top_k = top_k;
        Ok(self)
    }

    /// Replaces stop sequences; `None` and an explicit empty list remain distinct.
    pub fn with_stop(mut self, stop: Option<Vec<TextValue>>) -> Self {
        self.stop = stop;
        self
    }

    /// Replaces the deterministic sampling seed.
    pub fn with_seed(mut self, seed: Option<i64>) -> Self {
        self.seed = seed;
        self
    }

    /// Replaces repetition penalties after rejecting non-finite values.
    pub fn with_penalties(
        mut self,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
    ) -> Result<Self, SemanticValidationError> {
        self.frequency_penalty = frequency_penalty.map(FiniteF64::new).transpose()?;
        self.presence_penalty = presence_penalty.map(FiniteF64::new).transpose()?;
        Ok(self)
    }

    /// Replaces the effective parallel-function-call requirement.
    pub fn with_parallel_tool_calls(mut self, value: ParallelToolCalls) -> Self {
        self.parallel_tool_calls = value;
        self
    }

    /// Returns the requested output-token limit.
    pub const fn max_output_tokens(&self) -> Option<u64> {
        self.max_output_tokens
    }

    /// Returns the requested number of candidates.
    pub const fn candidate_count(&self) -> Option<u32> {
        self.candidate_count
    }

    /// Returns explicit temperature, if supplied.
    pub fn temperature(&self) -> Option<f64> {
        self.temperature.map(FiniteF64::get)
    }

    /// Returns explicit top-p, if supplied.
    pub fn top_p(&self) -> Option<f64> {
        self.top_p.map(FiniteF64::get)
    }

    /// Returns explicit top-k, if supplied.
    pub const fn top_k(&self) -> Option<u64> {
        self.top_k
    }

    /// Returns stop sequences while preserving omission versus an explicit empty list.
    pub fn stop(&self) -> Option<&[TextValue]> {
        self.stop.as_deref()
    }

    /// Returns the explicit sampling seed.
    pub const fn seed(&self) -> Option<i64> {
        self.seed
    }

    /// Returns the explicit frequency penalty.
    pub fn frequency_penalty(&self) -> Option<f64> {
        self.frequency_penalty.map(FiniteF64::get)
    }

    /// Returns the explicit presence penalty.
    pub fn presence_penalty(&self) -> Option<f64> {
        self.presence_penalty.map(FiniteF64::get)
    }

    /// Returns the effective parallel-function-call requirement.
    pub const fn parallel_tool_calls(&self) -> ParallelToolCalls {
        self.parallel_tool_calls
    }
}
