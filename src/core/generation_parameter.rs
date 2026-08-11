//! Typed catalog for recognized Chat Completions and Responses top-level request fields.
//!
//! The catalog classifies source-protocol field ownership and Bridge representability without
//! interpreting model capabilities or selecting Routes. Unknown names remain outside the type so
//! request analysis can reject them before Native preservation or protocol conversion.

use serde_json::{Map, Value};

use super::ApiProtocol;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FieldRole {
    Envelope,
    InterfaceParameter,
    RequestOption,
    ResponsesInclude,
    Streaming,
    ChatStreamOptions,
    Store,
    Background,
    PreviousResponseId,
}

/// Semantic Chat streaming-usage request after strict wire-shape validation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ChatStreamUsage {
    /// No usage tail is requested; omitted, empty, and explicit false shapes are equivalent.
    #[default]
    NotRequested,
    /// The client requires the standard usage-only chunk before `[DONE]`.
    Include,
}

impl ChatStreamUsage {
    /// Returns whether the downstream response must contain the Chat usage tail contract.
    pub(crate) const fn is_requested(self) -> bool {
        matches!(self, Self::Include)
    }
}

/// Parses the complete supported `stream_options` domain for one Chat request.
pub(crate) fn parse_chat_stream_usage(
    object: &Map<String, Value>,
    is_streaming: bool,
) -> Option<ChatStreamUsage> {
    let Some(value) = object.get("stream_options") else {
        return Some(ChatStreamUsage::NotRequested);
    };
    if !is_streaming {
        return None;
    }
    let options = value.as_object()?;
    match options.len() {
        0 => Some(ChatStreamUsage::NotRequested),
        1 => match options.get("include_usage")?.as_bool()? {
            true => Some(ChatStreamUsage::Include),
            false => Some(ChatStreamUsage::NotRequested),
        },
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProtocolSet {
    chat: bool,
    responses: bool,
}

const CHAT: ProtocolSet = ProtocolSet {
    chat: true,
    responses: false,
};
const RESPONSES: ProtocolSet = ProtocolSet {
    chat: false,
    responses: true,
};
const BOTH: ProtocolSet = ProtocolSet {
    chat: true,
    responses: true,
};
const NEITHER: ProtocolSet = ProtocolSet {
    chat: false,
    responses: false,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// One recognized top-level generation request field and its protocol boundaries.
pub(crate) struct GenerationRequestField {
    wire_name: &'static str,
    protocols: ProtocolSet,
    bridge_sources: ProtocolSet,
    role: FieldRole,
}

impl GenerationRequestField {
    /// Resolves one field only when it belongs to the selected downstream source protocol.
    pub(crate) fn from_wire(protocol: ApiProtocol, wire_name: &str) -> Option<Self> {
        GENERATION_REQUEST_FIELDS
            .iter()
            .copied()
            .find(|field| field.wire_name == wire_name && field.protocols.contains(protocol))
    }

    /// Resolves a canonical generation-model parameter independently of one wire protocol.
    pub(crate) fn from_model_parameter(wire_name: &str) -> Option<Self> {
        GENERATION_REQUEST_FIELDS.iter().copied().find(|field| {
            field.wire_name == wire_name
                && matches!(
                    field.role,
                    FieldRole::InterfaceParameter | FieldRole::ChatStreamOptions
                )
        })
    }

    /// Returns the stable top-level JSON field name.
    pub(crate) const fn as_wire_name(self) -> &'static str {
        self.wire_name
    }

    /// Returns whether a present value must belong to the fixed interface parameter contract.
    pub(crate) fn requires_interface_support(self, value: &Value) -> bool {
        match self.role {
            FieldRole::Envelope | FieldRole::Streaming => false,
            FieldRole::InterfaceParameter => true,
            FieldRole::ChatStreamOptions => {
                value
                    .as_object()
                    .and_then(|options| options.get("include_usage"))
                    .and_then(Value::as_bool)
                    == Some(true)
            }
            FieldRole::RequestOption => !value.is_null(),
            FieldRole::ResponsesInclude => {
                value.as_array().is_some_and(|values| !values.is_empty())
            }
            FieldRole::Store | FieldRole::Background => value.as_bool() == Some(true),
            FieldRole::PreviousResponseId => !value.is_null(),
        }
    }

    /// Returns whether the current Bridge direction explicitly represents this source field.
    pub(crate) const fn bridge_representable(self, protocol: ApiProtocol) -> bool {
        self.bridge_sources.contains(protocol)
    }

    /// Returns whether an otherwise unrepresentable state field carries a typed inactive value.
    pub(crate) fn bridge_inactive(self, value: &Value) -> bool {
        match self.role {
            FieldRole::Store => value.as_bool() == Some(false),
            FieldRole::Background => value.is_null() || value.as_bool() == Some(false),
            FieldRole::PreviousResponseId => value.is_null(),
            FieldRole::ResponsesInclude => {
                value.is_null() || value.as_array().is_some_and(Vec::is_empty)
            }
            FieldRole::RequestOption => value.is_null(),
            FieldRole::ChatStreamOptions => value.as_object().is_some_and(|options| {
                options.is_empty()
                    || (options.len() == 1
                        && options.get("include_usage").and_then(Value::as_bool) == Some(false))
            }),
            FieldRole::Envelope | FieldRole::InterfaceParameter | FieldRole::Streaming => false,
        }
    }
}

impl ProtocolSet {
    const fn contains(self, protocol: ApiProtocol) -> bool {
        match protocol {
            ApiProtocol::ChatCompletions => self.chat,
            ApiProtocol::Responses => self.responses,
        }
    }
}

const fn field(
    wire_name: &'static str,
    protocols: ProtocolSet,
    role: FieldRole,
    bridge_sources: ProtocolSet,
) -> GenerationRequestField {
    GenerationRequestField {
        wire_name,
        protocols,
        bridge_sources,
        role,
    }
}

const GENERATION_REQUEST_FIELDS: &[GenerationRequestField] = &[
    field("model", BOTH, FieldRole::Envelope, BOTH),
    field("messages", CHAT, FieldRole::Envelope, CHAT),
    field("input", RESPONSES, FieldRole::Envelope, RESPONSES),
    field("stream", BOTH, FieldRole::Streaming, BOTH),
    field("temperature", BOTH, FieldRole::InterfaceParameter, BOTH),
    field("top_p", BOTH, FieldRole::InterfaceParameter, BOTH),
    field(
        "frequency_penalty",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field(
        "presence_penalty",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("seed", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("n", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("logprobs", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("top_logprobs", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field(
        "include_reasoning",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("logit_bias", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("min_p", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("top_k", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("top_a", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field(
        "repetition_penalty",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("stop", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field(
        "structured_outputs",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("tools", BOTH, FieldRole::InterfaceParameter, BOTH),
    field("tool_choice", BOTH, FieldRole::InterfaceParameter, BOTH),
    field(
        "parallel_tool_calls",
        BOTH,
        FieldRole::InterfaceParameter,
        BOTH,
    ),
    field("max_tokens", CHAT, FieldRole::InterfaceParameter, CHAT),
    field(
        "max_completion_tokens",
        CHAT,
        FieldRole::InterfaceParameter,
        CHAT,
    ),
    field(
        "max_output_tokens",
        RESPONSES,
        FieldRole::InterfaceParameter,
        RESPONSES,
    ),
    field(
        "reasoning_effort",
        BOTH,
        FieldRole::InterfaceParameter,
        CHAT,
    ),
    field("reasoning", BOTH, FieldRole::InterfaceParameter, RESPONSES),
    field("response_format", CHAT, FieldRole::InterfaceParameter, CHAT),
    field("text", RESPONSES, FieldRole::InterfaceParameter, RESPONSES),
    field("store", BOTH, FieldRole::Store, NEITHER),
    field("background", RESPONSES, FieldRole::Background, NEITHER),
    field(
        "previous_response_id",
        RESPONSES,
        FieldRole::PreviousResponseId,
        NEITHER,
    ),
    field("metadata", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("service_tier", BOTH, FieldRole::InterfaceParameter, BOTH),
    field("user", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field(
        "safety_identifier",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field(
        "stream_options",
        CHAT,
        FieldRole::ChatStreamOptions,
        NEITHER,
    ),
    field("prompt_cache_key", BOTH, FieldRole::RequestOption, BOTH),
    field(
        "prompt_cache_options",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field(
        "prompt_cache_retention",
        BOTH,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("moderation", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("modalities", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("audio", BOTH, FieldRole::InterfaceParameter, NEITHER),
    field("asr_options", CHAT, FieldRole::InterfaceParameter, NEITHER),
    field(
        "optimize_text_preview",
        CHAT,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("prediction", CHAT, FieldRole::InterfaceParameter, NEITHER),
    field(
        "web_search_options",
        CHAT,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("functions", CHAT, FieldRole::InterfaceParameter, NEITHER),
    field(
        "function_call",
        CHAT,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("verbosity", CHAT, FieldRole::InterfaceParameter, NEITHER),
    field("instructions", RESPONSES, FieldRole::Envelope, RESPONSES),
    field(
        "conversation",
        RESPONSES,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("prompt", RESPONSES, FieldRole::InterfaceParameter, NEITHER),
    field(
        "context_management",
        RESPONSES,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field("include", RESPONSES, FieldRole::ResponsesInclude, RESPONSES),
    field(
        "truncation",
        RESPONSES,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
    field(
        "max_tool_calls",
        RESPONSES,
        FieldRole::InterfaceParameter,
        NEITHER,
    ),
];
