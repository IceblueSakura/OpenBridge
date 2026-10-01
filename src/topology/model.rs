//! Canonical semantic identity and public admission, independent of wire and Provider.
use super::{ModelId, TaskKind};
use crate::lowering::generation::{
    GenerationRepresentationContract as Representation, ReportedFactPolicy,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationSemanticContract {
    pub instructions: bool,
    pub temperature: bool,
    pub max_output_tokens: bool,
    pub tools: bool,
    pub custom_tools: bool,
    pub text_metadata: bool,
    pub top_p: bool,
    pub logprobs: bool,
    pub verbosity: bool,
    pub truncation: bool,
    pub structured_output: bool,
    pub reasoning: bool,
    pub image_input: bool,
    pub audio_input: bool,
    pub file_input: bool,
    pub parallel_tool_calls: bool,
    pub strict_tools: bool,
    pub cache_hints: bool,
    pub standard_context: bool,
}
impl GenerationSemanticContract {
    pub fn full() -> Self {
        Self::from_representation(&Representation::full())
    }
    pub fn text_images() -> Self {
        Self {
            image_input: true,
            audio_input: false,
            file_input: false,
            custom_tools: false,
            strict_tools: false,
            cache_hints: false,
            standard_context: false,
            ..Self::full()
        }
    }
    /// Shared requirement checking uses only these semantic flags, never a profile/scope.
    pub(crate) fn representation(&self) -> Representation {
        Representation {
            instructions: self.instructions,
            temperature: self.temperature,
            max_output_tokens: self.max_output_tokens,
            tools: self.tools,
            custom_tools: self.custom_tools,
            text_metadata: self.text_metadata,
            top_p: self.top_p,
            logprobs: self.logprobs,
            verbosity: self.verbosity,
            truncation: self.truncation,
            structured_output: self.structured_output,
            reasoning: self.reasoning,
            image_input: self.image_input,
            audio_input: self.audio_input,
            file_input: self.file_input,
            parallel_tool_calls: self.parallel_tool_calls,
            strict_tools: self.strict_tools,
            cache_hints: self.cache_hints,
            standard_context: self.standard_context,
            ..Representation::full()
        }
    }
    /// Explicit boundary conversion; adapters and replay/delivery policies are not copied.
    pub fn from_representation(value: &Representation) -> Self {
        Self {
            instructions: value.instructions,
            temperature: value.temperature,
            max_output_tokens: value.max_output_tokens,
            tools: value.tools,
            custom_tools: value.custom_tools,
            text_metadata: value.text_metadata,
            top_p: value.top_p,
            logprobs: value.logprobs,
            verbosity: value.verbosity,
            truncation: value.truncation,
            structured_output: value.structured_output,
            reasoning: value.reasoning,
            image_input: value.image_input,
            audio_input: value.audio_input,
            file_input: value.file_input,
            parallel_tool_calls: value.parallel_tool_calls,
            strict_tools: value.strict_tools,
            cache_hints: value.cache_hints,
            standard_context: value.standard_context,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalModel {
    pub id: ModelId,
    pub task: TaskKind,
    pub contract: GenerationSemanticContract,
}
impl super::PublicModel {
    /// An alias may intentionally expose a narrower semantic slice of one canonical model.
    pub fn new(
        id: ModelId,
        canonical_model: ModelId,
        task: TaskKind,
        route: super::RouteId,
        contract: GenerationSemanticContract,
    ) -> Self {
        Self {
            id,
            canonical_model,
            task,
            route,
            contract,
            reported_facts: ReportedFactPolicy::Faithful,
        }
    }
}
