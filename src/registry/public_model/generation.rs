//! Generation interface DTO, serialization, and request-time accessors.

use super::*;

/// Unique, fixed capability contract for one protocol interface, used directly by request preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInterfaceCapabilities {
    pub(super) context_window: ContextWindow,
    pub(super) modalities: ModelModalities,
    pub(super) media: InterfaceMediaCapabilities,
    pub(super) supported_parameters: Vec<String>,
    pub(super) streaming: SupportState,
    pub(super) non_streaming: SupportState,
    pub(super) system_messages: SupportState,
    pub(super) tools: ToolCapabilities,
    pub(super) structured_outputs: Option<StructuredOutputProfile>,
    pub(super) reasoning: InterfaceReasoningCapabilities,
    pub(super) response_includes: Vec<ResponseInclude>,
    pub(super) state: StateCapabilities,
}

/// Transient Models projection derived from the closed execution profile.
#[derive(Serialize)]
struct StructuredOutputCapabilitiesWire {
    pub(super) support: SupportState,
    pub(super) modes: &'static [StructuredOutputMode],
    pub(super) strict_schema: SupportState,
}

impl From<Option<StructuredOutputProfile>> for StructuredOutputCapabilitiesWire {
    /// Projects the execution profile without retaining independently mutable DTO state.
    fn from(profile: Option<StructuredOutputProfile>) -> Self {
        match profile {
            Some(profile) => Self {
                support: SupportState::Supported,
                modes: profile.modes(),
                strict_schema: SupportState::from_bool(profile.supports_strict_schema()),
            },
            None => Self {
                support: SupportState::Unsupported,
                modes: &[],
                strict_schema: SupportState::Unsupported,
            },
        }
    }
}

/// Borrowed wire projection that preserves the established Models extension layout.
#[derive(Serialize)]
struct ModelInterfaceCapabilitiesWire<'a> {
    pub(super) context_window: &'a ContextWindow,
    pub(super) modalities: &'a ModelModalities,
    pub(super) multimodal_input: MultimodalInputCapabilities,
    pub(super) multimodal_output: MultimodalOutputCapabilities,
    pub(super) audio_task: Option<AudioTaskProjection>,
    pub(super) supported_parameters: &'a [String],
    pub(super) streaming: SupportState,
    pub(super) non_streaming: SupportState,
    pub(super) system_messages: SupportState,
    pub(super) tools: &'a ToolCapabilities,
    pub(super) structured_outputs: StructuredOutputCapabilitiesWire,
    pub(super) reasoning: &'a InterfaceReasoningCapabilities,
    pub(super) response_includes: &'a [ResponseInclude],
    pub(super) state: &'a StateCapabilities,
}

impl Serialize for ModelInterfaceCapabilities {
    /// Serializes the private unions through the stable downstream-safe projection only.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Derive private execution unions into transient downstream-safe wire projections.
        let (audio, voice_conditioning, audio_output, audio_task) = self
            .media
            .audio
            .as_ref()
            .map_or((None, None, None, None), |audio| {
                let (input, conditioning) = audio.multimodal_input();
                (
                    input,
                    conditioning,
                    audio.multimodal_output(),
                    Some(audio.task_projection()),
                )
            });
        let structured_outputs = StructuredOutputCapabilitiesWire::from(self.structured_outputs);

        // Build the stable wire object only after deriving every transient capability projection.
        ModelInterfaceCapabilitiesWire {
            context_window: &self.context_window,
            modalities: &self.modalities,
            multimodal_input: MultimodalInputCapabilities {
                image: self.media.image.clone(),
                audio,
                voice_conditioning,
                file: self.media.file.clone(),
            },
            multimodal_output: MultimodalOutputCapabilities {
                audio: audio_output,
            },
            audio_task,
            supported_parameters: &self.supported_parameters,
            streaming: self.streaming,
            non_streaming: self.non_streaming,
            system_messages: self.system_messages,
            tools: &self.tools,
            structured_outputs,
            reasoning: &self.reasoning,
            response_includes: &self.response_includes,
            state: &self.state,
        }
        .serialize(serializer)
    }
}

impl ModelInterfaceCapabilities {
    /// Returns the fixed typed file contract used by request preflight.
    pub(crate) const fn file_input(&self) -> Option<&FileInputInterfaceCapabilities> {
        self.media.file.as_ref()
    }

    /// Returns whether this generation interface accepts one optional top-level request parameter.
    pub(crate) fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|supported| supported == parameter)
    }

    /// Returns whether this Responses interface guarantees one additional output projection.
    pub(crate) fn supports_response_include(&self, include: ResponseInclude) -> bool {
        self.response_includes.contains(&include)
    }

    /// Returns whether the interface guarantees streaming support.
    pub(crate) const fn supports_streaming(&self) -> bool {
        self.streaming.is_supported()
    }

    /// Returns whether the interface guarantees one complete non-streaming JSON response.
    pub(crate) const fn supports_non_streaming(&self) -> bool {
        self.non_streaming.is_supported()
    }

    /// Returns whether the interface guarantees standard function-tool declarations.
    pub(crate) const fn supports_function_tools(&self) -> bool {
        self.tools.support.is_supported()
    }

    /// Returns whether the interface guarantees one function-tool choice mode.
    pub(crate) fn supports_tool_choice(&self, mode: ToolChoiceMode) -> bool {
        self.tools.support.is_supported() && self.tools.tool_choice_modes.contains(&mode)
    }

    /// Returns whether the interface guarantees parallel function calls.
    pub(crate) const fn supports_parallel_tool_calls(&self) -> bool {
        self.tools.parallel_calls.is_supported()
    }

    /// Returns whether strict function-tool JSON Schema is guaranteed.
    pub(crate) const fn supports_strict_tool_schema(&self) -> bool {
        self.tools.strict_schema.is_supported()
    }

    /// Returns the typed image-input profile guaranteed by every interface candidate.
    pub(crate) fn image_input(&self) -> Option<&ImageInputInterfaceCapabilities> {
        self.media.image.as_ref()
    }

    /// Returns the closed audio contract guaranteed by every interface candidate.
    pub(crate) const fn audio(&self) -> Option<&AudioInterfaceCapabilities> {
        self.media.audio.as_ref()
    }

    /// Returns the closed structured-output profile guaranteed by every interface candidate.
    pub(crate) const fn structured_outputs(&self) -> Option<StructuredOutputProfile> {
        self.structured_outputs
    }

    /// Returns whether the interface guarantees background responses.
    pub(crate) const fn supports_background(&self) -> bool {
        self.state.background.is_supported()
    }

    /// Returns the maximum output-token count guaranteed by the interface.
    pub(crate) const fn max_output_tokens(&self) -> Option<u32> {
        self.context_window.max_output_tokens()
    }

    /// Returns the interface's reasoning evidence state.
    pub(crate) const fn reasoning_support(&self) -> SupportState {
        self.reasoning.support
    }

    /// Resolves one requested reasoning level against the fixed interface input policy.
    pub(crate) fn resolve_reasoning_level(
        &self,
        requested: ReasoningLevel,
    ) -> Option<ReasoningLevel> {
        // Unknown or unsupported reasoning cannot acquire an executable level through normalization.
        if !self.reasoning.support.is_supported() {
            return None;
        }

        // Apply only the Public Model policy compiled beside the executable level intersection.
        self.reasoning
            .input_policy
            .resolve(requested, &self.reasoning.levels)
    }
}
