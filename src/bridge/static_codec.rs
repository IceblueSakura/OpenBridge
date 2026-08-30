//! Static Chat/Responses codecs that pass model-interaction semantics through Generation IR.
//!
//! R2 keeps this facade separate from the production `BridgePlan`: tests dual-run both paths until
//! Event IR exists and the Bridge can switch atomically. Wire delivery metadata remains outside the
//! canonical IR, while request and response interaction semantics are decoded, validated, and
//! encoded by pure functions.

use bytes::Bytes;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ApiRequest, GenerationRequestField, ReasoningOutput},
    ir::generation::{GenerationRequest, GenerationResponse, SemanticChange},
};

mod request;
mod response;

/// A decoded request plus protocol delivery fields that are not model-interaction semantics.
#[derive(Debug)]
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

/// Immutable non-stream Static IR conversion plan used by R2 parity tests.
#[derive(Debug)]
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
        if request.stream == Some(true) {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        let target = request::lower_request(
            target_protocol,
            &request,
            upstream_model,
            reasoning_output == ReasoningOutput::Summary,
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
