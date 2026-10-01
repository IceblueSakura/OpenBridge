//! Task admission vocabulary. No wire profiles, routing, credentials or cache policy.
use super::{GenerationRequest, GenerationRequirements};
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum GenerationFeature {
    Instructions,
    Temperature,
    MaxOutputTokens,
    Tools,
    TextMetadata,
    Controls,
    ParallelTools,
    StrictTools,
    StructuredOutput,
    Reasoning,
    ImageInput,
    AudioInput,
    FileInput,
}
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
}
impl GenerationSemanticContract {
    /// Complete vocabulary, useful for codecs and synthetic fixtures, not model discovery.
    pub fn full() -> Self {
        Self {
            instructions: true,
            temperature: true,
            max_output_tokens: true,
            tools: true,
            custom_tools: true,
            text_metadata: true,
            top_p: true,
            logprobs: true,
            verbosity: true,
            truncation: true,
            structured_output: true,
            reasoning: true,
            image_input: true,
            audio_input: true,
            file_input: true,
            parallel_tool_calls: true,
            strict_tools: true,
        }
    }
    /// Explicit currently admitted text/image slice; new fields require a decision here.
    pub fn text_images() -> Self {
        Self {
            instructions: true,
            temperature: true,
            max_output_tokens: true,
            tools: true,
            custom_tools: false,
            text_metadata: true,
            top_p: true,
            logprobs: true,
            verbosity: true,
            truncation: true,
            structured_output: true,
            reasoning: true,
            image_input: true,
            audio_input: false,
            file_input: false,
            parallel_tool_calls: true,
            strict_tools: false,
        }
    }
    pub fn supports(&self, promise: &Self) -> bool {
        [
            (promise.instructions, self.instructions),
            (promise.temperature, self.temperature),
            (promise.max_output_tokens, self.max_output_tokens),
            (promise.tools, self.tools),
            (promise.custom_tools, self.custom_tools),
            (promise.text_metadata, self.text_metadata),
            (promise.top_p, self.top_p),
            (promise.logprobs, self.logprobs),
            (promise.verbosity, self.verbosity),
            (promise.truncation, self.truncation),
            (promise.structured_output, self.structured_output),
            (promise.reasoning, self.reasoning),
            (promise.image_input, self.image_input),
            (promise.audio_input, self.audio_input),
            (promise.file_input, self.file_input),
            (promise.parallel_tool_calls, self.parallel_tool_calls),
            (promise.strict_tools, self.strict_tools),
        ]
        .into_iter()
        .all(|(required, supported)| !required || supported)
    }
    /// Validation remains owned by GenerationRequest; this checks semantic support only.
    pub fn check(
        &self,
        r: &GenerationRequest,
    ) -> Result<GenerationRequirements, GenerationFeature> {
        use GenerationFeature::*;
        let q = GenerationRequirements::derive(r);
        for (required, supported, feature) in [
            (q.instruction_count > 0, self.instructions, Instructions),
            (q.temperature, self.temperature, Temperature),
            (
                q.max_output_tokens.is_some(),
                self.max_output_tokens,
                MaxOutputTokens,
            ),
            (
                q.tool_count > 0
                    || q.tool_history
                    || q.tool_choice.is_some()
                    || q.parallel_tool_calls.is_some(),
                self.tools,
                Tools,
            ),
            (q.custom_tools, self.custom_tools, Tools),
            (q.text_metadata, self.text_metadata, TextMetadata),
            (q.top_p, self.top_p, Controls),
            (q.logprobs, self.logprobs, Controls),
            (q.truncation, self.truncation, Controls),
            (
                !r.text_options().verbosity.is_absent(),
                self.verbosity,
                Controls,
            ),
            (
                q.parallel_tool_calls == Some(true),
                self.parallel_tool_calls,
                ParallelTools,
            ),
            (q.strict_function_tools, self.strict_tools, StrictTools),
            (
                q.structured_output,
                self.structured_output,
                StructuredOutput,
            ),
            (q.reasoning, self.reasoning, Reasoning),
            (q.image_inputs > 0, self.image_input, ImageInput),
            (q.audio_inputs > 0, self.audio_input, AudioInput),
            (q.file_inputs > 0, self.file_input, FileInput),
        ] {
            if required && !supported {
                return Err(feature);
            }
        }
        Ok(q)
    }
}
