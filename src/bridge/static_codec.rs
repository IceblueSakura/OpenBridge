//! Production Chat/Responses Bridge backed exclusively by canonical Generation IR.
//!
//! Wire delivery metadata remains outside the canonical IR. Request and response interaction
//! semantics are decoded, validated, and encoded by pure functions; incremental SSE conversion is
//! delegated to the canonical Event reducer/encoder without owning transport or commit policy.

use bytes::Bytes;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    core::{
        ApiProtocol, ApiRequest, ChatStreamUsage, GenerationRequestField, ReasoningOutput,
        parse_chat_stream_usage,
    },
    ir::generation::{
        EventLimits, GenerationRequest, GenerationResponse, LossPolicy, ProviderToolProfile,
        SemanticChange, ToolPlan, apply_tool_plan, enforce_loss_policy,
    },
    transport::sse::SseEvent,
};

use super::event_codec::{StaticEventBridge, StaticEventCodecError};

mod request;
mod response;

/// A decoded request plus protocol delivery fields that are not model-interaction semantics.
#[derive(Clone, Debug)]
struct WireRequest {
    semantic: GenerationRequest,
    stream: Option<bool>,
    service_tier: Option<String>,
}

/// A decoded response plus its source wire identity for downstream ID rendering.
struct WireResponse {
    semantic: GenerationResponse,
    source_id: String,
}

/// Static codec rejection before production Bridge takeover.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StaticCodecError {
    /// A request or response body exceeds the caller-approved admission limit.
    #[error("static generation codec body exceeds the configured limit")]
    LimitExceeded,
    /// JSON or one required protocol field has an invalid shape.
    #[error("static generation codec received an invalid wire shape")]
    InvalidShape,
    /// The accepted wire value has no exact or authorized target representation.
    #[error("static generation codec cannot preserve the requested semantics")]
    UnsupportedSemantics,
    /// Function arguments are not one complete JSON object.
    #[error("static generation codec received invalid function arguments")]
    InvalidToolArguments,
    /// Tool calls/results contain a duplicate or unresolved identity.
    #[error("static generation codec received an invalid tool identity")]
    InvalidToolIdentity,
}

impl StaticCodecError {
    fn from_validation<T>(_error: T) -> Self {
        Self::InvalidShape
    }
}

/// Caller-approved request and response admission bounds for the pure static codecs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticCodecLimits {
    request_body: usize,
    response_body: usize,
}

impl StaticCodecLimits {
    /// Creates non-zero body limits; leaf-value bounds reuse the owning body boundary.
    pub fn new(request_body: usize, response_body: usize) -> Result<Self, StaticCodecError> {
        if request_body == 0 || response_body == 0 {
            return Err(StaticCodecError::InvalidShape);
        }
        Ok(Self {
            request_body,
            response_body,
        })
    }
}

/// Trusted Provider-native tool target fixed before request lowering.
#[derive(Clone, Copy, Debug)]
pub struct ProviderToolTarget<'a> {
    tool_plan: &'a ToolPlan,
    provider_profile: &'a ProviderToolProfile,
    reasoning_output: ReasoningOutput,
}

impl<'a> ProviderToolTarget<'a> {
    /// Binds one immutable plan to one fixed Provider origin and reasoning profile.
    pub const fn new(
        tool_plan: &'a ToolPlan,
        provider_profile: &'a ProviderToolProfile,
        reasoning_output: ReasoningOutput,
    ) -> Self {
        Self {
            tool_plan,
            provider_profile,
            reasoning_output,
        }
    }
}

/// Error returned when canonical request, response, or Event conversion cannot preserve the Bridge contract.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BridgeError {
    /// Input JSON or a required protocol field has an invalid shape.
    #[error("bridge input is not a valid protocol object")]
    InvalidShape,
    /// The input uses semantics without an exact or authorized target representation.
    #[error("bridge input uses unsupported semantics")]
    UnsupportedSemantics,
    /// A tool identity is duplicated, missing, or unresolved.
    #[error("bridge tool identity is invalid")]
    InvalidToolIdentity,
    /// Function arguments are not one complete JSON object.
    #[error("bridge function arguments are invalid")]
    InvalidToolArguments,
    /// Request, response, Event, or encoded target output exceeds an approved bound.
    #[error("bridge conversion exceeds the configured limit")]
    LimitExceeded,
    /// The upstream Event lifecycle, identity, terminal, or EOF contract is invalid.
    #[error("bridge stream lifecycle is invalid")]
    InvalidStream,
}

impl From<StaticCodecError> for BridgeError {
    fn from(error: StaticCodecError) -> Self {
        match error {
            StaticCodecError::LimitExceeded => Self::LimitExceeded,
            StaticCodecError::InvalidShape => Self::InvalidShape,
            StaticCodecError::UnsupportedSemantics => Self::UnsupportedSemantics,
            StaticCodecError::InvalidToolArguments => Self::InvalidToolArguments,
            StaticCodecError::InvalidToolIdentity => Self::InvalidToolIdentity,
        }
    }
}

impl From<StaticEventCodecError> for BridgeError {
    fn from(error: StaticEventCodecError) -> Self {
        match error {
            StaticEventCodecError::LimitExceeded => Self::LimitExceeded,
            StaticEventCodecError::UnsupportedSemantics => Self::UnsupportedSemantics,
            StaticEventCodecError::InvalidJson => Self::InvalidShape,
            StaticEventCodecError::InvalidLifecycle
            | StaticEventCodecError::IdentityConflict
            | StaticEventCodecError::DuplicateIdentity
            | StaticEventCodecError::EofBeforeTerminal
            | StaticEventCodecError::Reduce(_)
            | StaticEventCodecError::Materialize(_) => Self::InvalidStream,
        }
    }
}

/// Caller-approved request, JSON response, SSE event, and aggregate Event bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeLimits {
    static_limits: StaticCodecLimits,
    event_limits: EventLimits,
}

impl BridgeLimits {
    /// Creates one Bridge budget from the runtime request and Generation response limits.
    pub fn new(
        max_request_body_bytes: usize,
        max_json_response_body_bytes: usize,
        max_sse_event_bytes: usize,
    ) -> Result<Self, BridgeError> {
        let static_limits =
            StaticCodecLimits::new(max_request_body_bytes, max_json_response_body_bytes)?;
        let event_limits = EventLimits::new(
            max_sse_event_bytes,
            max_json_response_body_bytes,
            max_json_response_body_bytes,
        )
        .map_err(|_| BridgeError::InvalidShape)?;
        Ok(Self {
            static_limits,
            event_limits,
        })
    }
}

/// Immutable Static IR request/response conversion plan.
#[derive(Clone, Debug)]
pub struct StaticBridgePlan {
    target_protocol: ApiProtocol,
    public_model: String,
    reasoning_output: ReasoningOutput,
    limits: StaticCodecLimits,
    request: WireRequest,
    request_changes: Vec<SemanticChange>,
}

/// Encoded non-stream response plus the observable lowering fidelity report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticRenderedResponse {
    body: Bytes,
    changes: Vec<SemanticChange>,
    semantic: GenerationResponse,
}

impl StaticRenderedResponse {
    /// Returns the compact target response body.
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    /// Returns semantic changes observed during target lowering.
    pub fn changes(&self) -> &[SemanticChange] {
        &self.changes
    }

    /// Returns the canonical source response decoded before target lowering.
    pub fn semantic(&self) -> &GenerationResponse {
        &self.semantic
    }
}

impl StaticBridgePlan {
    /// Decodes one supported source request and fixes the target wire profile.
    pub fn prepare(
        source_protocol: ApiProtocol,
        target_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        limits: StaticCodecLimits,
    ) -> Result<(Self, ApiRequest), StaticCodecError> {
        Self::prepare_with_reasoning_output(
            source_protocol,
            target_protocol,
            public_model,
            upstream_model,
            body,
            ReasoningOutput::Unsupported,
            limits,
        )
    }

    /// Decodes one request with an explicit readable-reasoning contract.
    pub fn prepare_with_reasoning_output(
        source_protocol: ApiProtocol,
        target_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        reasoning_output: ReasoningOutput,
        limits: StaticCodecLimits,
    ) -> Result<(Self, ApiRequest), StaticCodecError> {
        if source_protocol == target_protocol
            || public_model.is_empty()
            || upstream_model.is_empty()
        {
            return Err(StaticCodecError::InvalidShape);
        }
        if body.len() > limits.request_body {
            return Err(StaticCodecError::LimitExceeded);
        }
        let source = parse_object(&body)?;
        validate_source(source_protocol, &source)?;
        let request = request::decode_request(source_protocol, &source, limits.request_body)?;
        let target = request::lower_request(
            target_protocol,
            &request,
            upstream_model,
            reasoning_output == ReasoningOutput::Summary,
            None,
        )?;
        let (target, request_changes) = target.into_parts();
        let target = request::encode_request(target)?;
        if target.len() > limits.request_body {
            return Err(StaticCodecError::LimitExceeded);
        }
        Ok((
            Self {
                target_protocol,
                public_model: public_model.to_owned(),
                reasoning_output,
                limits,
                request,
                request_changes,
            },
            ApiRequest::new(target_protocol, target),
        ))
    }

    /// Applies one trusted ToolPlan before lowering to a fixed Provider-native target profile.
    pub fn prepare_with_tool_plan(
        source_protocol: ApiProtocol,
        target_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        tool_target: ProviderToolTarget<'_>,
        limits: StaticCodecLimits,
    ) -> Result<(Self, ApiRequest), StaticCodecError> {
        if source_protocol == target_protocol
            || public_model.is_empty()
            || upstream_model.is_empty()
        {
            return Err(StaticCodecError::InvalidShape);
        }
        if body.len() > limits.request_body {
            return Err(StaticCodecError::LimitExceeded);
        }
        let source = parse_object(&body)?;
        validate_source(source_protocol, &source)?;
        let mut request = request::decode_request(source_protocol, &source, limits.request_body)?;
        let transformed = apply_tool_plan(request.semantic, tool_target.tool_plan)
            .map_err(|_| StaticCodecError::UnsupportedSemantics)?;
        enforce_loss_policy(&transformed, LossPolicy::Reject)
            .map_err(|_| StaticCodecError::UnsupportedSemantics)?;
        let (semantic, mut request_changes) = transformed.into_parts();
        request.semantic = semantic;
        let target = request::lower_request(
            target_protocol,
            &request,
            upstream_model,
            tool_target.reasoning_output == ReasoningOutput::Summary,
            Some(tool_target.provider_profile),
        )?;
        let (target, lowering_changes) = target.into_parts();
        request_changes.extend(lowering_changes);
        let target = request::encode_request(target)?;
        if target.len() > limits.request_body {
            return Err(StaticCodecError::LimitExceeded);
        }
        Ok((
            Self {
                target_protocol,
                public_model: public_model.to_owned(),
                reasoning_output: tool_target.reasoning_output,
                limits,
                request,
                request_changes,
            },
            ApiRequest::new(target_protocol, target),
        ))
    }

    /// Decodes a complete target response into Static IR and renders the downstream protocol.
    pub fn render_non_stream(
        &self,
        body: Bytes,
    ) -> Result<StaticRenderedResponse, StaticCodecError> {
        if body.len() > self.limits.response_body {
            return Err(StaticCodecError::LimitExceeded);
        }
        let source = parse_object(&body)?;
        let decoded = response::decode_response(
            self.target_protocol,
            &source,
            self.reasoning_output,
            self.limits.response_body,
        )?;
        let semantic = decoded.semantic.clone();
        let target_protocol = opposite(self.target_protocol);
        let rendered = response::lower_response(
            target_protocol,
            &decoded,
            &self.public_model,
            self.reasoning_output,
        )?;
        let (rendered, changes) = rendered.into_parts();
        let rendered = response::encode_response(rendered)?;
        if rendered.len() > self.limits.response_body {
            return Err(StaticCodecError::LimitExceeded);
        }
        Ok(StaticRenderedResponse {
            body: rendered,
            changes,
            semantic,
        })
    }

    /// Returns the canonical request, primarily for semantic parity assertions during R2.
    pub fn request(&self) -> &GenerationRequest {
        &self.request.semantic
    }

    /// Returns semantic changes observed while lowering the request target DTO.
    pub fn request_changes(&self) -> &[SemanticChange] {
        &self.request_changes
    }
}

/// Production Bridge plan with one canonical Static request/response plan and fixed Event profile.
#[derive(Clone, Debug)]
pub struct BridgePlan {
    downstream_protocol: ApiProtocol,
    upstream_protocol: ApiProtocol,
    reasoning_output: ReasoningOutput,
    chat_stream_usage: ChatStreamUsage,
    limits: BridgeLimits,
    static_plan: StaticBridgePlan,
}

impl BridgePlan {
    /// Decodes and lowers one downstream request using explicit runtime bounds.
    pub fn prepare(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        limits: BridgeLimits,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        Self::prepare_with_reasoning_output(
            downstream_protocol,
            upstream_protocol,
            public_model,
            upstream_model,
            body,
            ReasoningOutput::Unsupported,
            limits,
        )
    }

    /// Decodes and lowers one request with an explicit readable-reasoning contract.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_reasoning_output(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        reasoning_output: ReasoningOutput,
        limits: BridgeLimits,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        if body.len() > limits.static_limits.request_body {
            return Err(BridgeError::LimitExceeded);
        }
        let source = parse_object(&body)?;
        let chat_stream_usage = direct_chat_stream_usage(downstream_protocol, &source)?;
        if reasoning_output == ReasoningOutput::Unsupported
            && match downstream_protocol {
                ApiProtocol::ChatCompletions => source
                    .get("reasoning_effort")
                    .is_some_and(|value| !value.is_null()),
                ApiProtocol::Responses => source
                    .get("reasoning")
                    .is_some_and(|value| !value.is_null()),
            }
        {
            return Err(BridgeError::UnsupportedSemantics);
        }
        Self::prepare_with_request_facts(
            downstream_protocol,
            upstream_protocol,
            public_model,
            upstream_model,
            body,
            reasoning_output,
            chat_stream_usage,
            limits,
        )
    }

    /// Prepares a Bridge from request facts already frozen by production analysis.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_with_request_facts(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
        reasoning_output: ReasoningOutput,
        chat_stream_usage: ChatStreamUsage,
        limits: BridgeLimits,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        let body = if downstream_protocol == ApiProtocol::ChatCompletions {
            let mut source = parse_object(&body)?;
            source.remove("stream_options");
            Bytes::from(
                serde_json::to_vec(&Value::Object(source))
                    .map_err(|_| BridgeError::InvalidShape)?,
            )
        } else {
            body
        };
        let (static_plan, request) = StaticBridgePlan::prepare_with_reasoning_output(
            downstream_protocol,
            upstream_protocol,
            public_model,
            upstream_model,
            body,
            reasoning_output,
            limits.static_limits,
        )?;
        Ok((
            Self {
                downstream_protocol,
                upstream_protocol,
                reasoning_output,
                chat_stream_usage,
                limits,
                static_plan,
            },
            request,
        ))
    }

    /// Returns the downstream protocol fixed during request planning.
    pub const fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_protocol
    }

    /// Returns the upstream protocol fixed during request planning.
    pub const fn upstream_protocol(&self) -> ApiProtocol {
        self.upstream_protocol
    }

    /// Converts one complete successful upstream response through canonical Static IR.
    pub fn render_non_stream(&self, body: Bytes) -> Result<Bytes, BridgeError> {
        self.render_non_stream_ir(body)
            .map(|rendered| rendered.body().clone())
    }

    /// Returns the complete canonical response and fidelity report for contract verification.
    pub fn render_non_stream_ir(&self, body: Bytes) -> Result<StaticRenderedResponse, BridgeError> {
        self.static_plan.render_non_stream(body).map_err(Into::into)
    }

    /// Creates an incremental Event IR renderer dedicated to this request.
    pub fn stream_renderer(&self) -> BridgeStreamRenderer {
        let inner = StaticEventBridge::new(
            self.upstream_protocol,
            self.downstream_protocol,
            &self.static_plan.public_model,
            self.reasoning_output,
            self.chat_stream_usage.is_requested(),
            self.limits.event_limits,
        )
        .expect("validated BridgePlan always has opposite protocols and non-empty model");
        BridgeStreamRenderer { inner }
    }

    /// Returns the canonical request fixed before Provider selection.
    pub fn request(&self) -> &GenerationRequest {
        self.static_plan.request()
    }

    /// Returns request-lowering fidelity changes.
    pub fn request_changes(&self) -> &[SemanticChange] {
        self.static_plan.request_changes()
    }
}

/// Incremental production SSE renderer backed by one canonical Event IR state.
pub struct BridgeStreamRenderer {
    inner: StaticEventBridge,
}

impl BridgeStreamRenderer {
    /// Reduces one complete upstream SSE event and emits zero or more downstream events.
    pub fn render(&mut self, event: SseEvent) -> Result<Bytes, BridgeError> {
        self.inner.render(event).map_err(Into::into)
    }

    /// Applies EOF and requires a valid explicit terminal.
    pub fn finish(&mut self) -> Result<Bytes, BridgeError> {
        self.inner.finish().map_err(Into::into)
    }
}

fn direct_chat_stream_usage(
    downstream_protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<ChatStreamUsage, BridgeError> {
    if downstream_protocol != ApiProtocol::ChatCompletions {
        return Ok(ChatStreamUsage::NotRequested);
    }
    let streaming = source.get("stream").and_then(Value::as_bool) == Some(true);
    parse_chat_stream_usage(source, streaming).ok_or(BridgeError::UnsupportedSemantics)
}

fn parse_object(body: &[u8]) -> Result<Map<String, Value>, StaticCodecError> {
    serde_json::from_slice::<Value>(body)
        .map_err(|_| StaticCodecError::InvalidShape)?
        .as_object()
        .cloned()
        .ok_or(StaticCodecError::InvalidShape)
}

fn validate_source(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), StaticCodecError> {
    if source
        .get("model")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(StaticCodecError::InvalidShape);
    }
    if source.iter().any(|(wire_name, value)| {
        if protocol == ApiProtocol::Responses && wire_name == "text" {
            return false;
        }
        GenerationRequestField::from_wire(protocol, wire_name).is_none_or(|field| {
            !field.bridge_representable(protocol) && !field.bridge_inactive(value)
        })
    }) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    if protocol == ApiProtocol::ChatCompletions && source.contains_key("functions") {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    if source
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.get("type").and_then(Value::as_str) != Some("function"))
        })
    {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    Ok(())
}

const fn opposite(protocol: ApiProtocol) -> ApiProtocol {
    match protocol {
        ApiProtocol::ChatCompletions => ApiProtocol::Responses,
        ApiProtocol::Responses => ApiProtocol::ChatCompletions,
    }
}
