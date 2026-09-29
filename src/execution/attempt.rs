//! Attempt preparation, response intake and downstream rendering over
//! semantically blind transport edges.
//!
//! The chain is symmetric: upstream bytes decode through the endpoint protocol
//! codec into semantic IR, and downstream bytes are produced only from validated
//! IR. Nothing here reads credentials from topology, follows redirects or
//! retries; the delivery lifecycle owns the commit boundary.

use crate::{
    execution::lifecycle::{Lifecycle, LifecycleError},
    lowering::generation::{
        GenerationRepresentationContract, RepresentationError, check, lower_request, lower_response,
    },
    protocol::{
        extensions::CustomSections,
        openai::{
            CodecError, DecodedRequest, DecodedResponse, Profile, ResponseMetadata,
            chat_envelope::{self, StreamOptions},
            chat_sse::{ChatSseDecoder, ChatSseEncoder},
            envelope,
            sse::{Obfuscation, ResponsesSseDecoder, ResponsesSseEncoder, SseError, SseLimits},
        },
    },
    provider::{ErrorClass, ProviderDefinition, SecretMaterial, StatusClass, classify_status},
    semantic::{task::generation::*, value::Presence},
    topology::{Endpoint, ProtocolProfile, PublicModel},
};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum AttemptError {
    #[error(transparent)]
    Representation(#[from] RepresentationError),
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error(transparent)]
    Sse(#[from] SseError),
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    #[error("upstream returned HTTP {status}")]
    Status { status: u16, class: ErrorClass },
    #[error("transport failure")]
    Transport(ErrorClass),
    #[error("protocol boundary: {0}")]
    Protocol(&'static str),
    #[error("bounded body or stream budget exceeded")]
    Limit,
    #[error("delivery mismatch: {0}")]
    Delivery(&'static str),
}

/// Upstream HTTP request parts. The transport edge receives the trusted target,
/// safe headers and sensitive auth material separately and never sees task IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamRequest {
    pub origin: String,
    pub method: &'static str,
    pub path: String,
    pub safe_headers: Vec<(String, String)>,
    pub auth_header: (String, String),
    pub body: Vec<u8>,
}

fn codec_profile(protocol: ProtocolProfile) -> Profile {
    match protocol {
        ProtocolProfile::OpenAiChat => Profile::Chat,
        ProtocolProfile::OpenAiResponses => Profile::Responses,
    }
}

fn presence<T>(value: Option<T>) -> Presence<T> {
    match value {
        Some(value) => Presence::Value(value),
        None => Presence::Absent,
    }
}

/// Entry admission: the request must fit the public contract of its model label.
pub fn admit(
    public: &PublicModel,
    request: &DecodedRequest,
) -> Result<GenerationRequirements, AttemptError> {
    Ok(check(&request.semantic, public.contract.clone())?)
}

/// Normalized downstream request context across chat and responses envelopes.
#[derive(Clone, Debug)]
struct WireContext {
    source: ProtocolProfile,
    stream: Presence<bool>,
    chat_options: Option<StreamOptions>,
    responses_options: Option<envelope::StreamOptions>,
    cache: envelope::CacheHints,
    execution: envelope::ExecutionHints,
    n: Presence<u64>,
    extensions: CustomSections,
}

impl WireContext {
    fn from_chat(context: &chat_envelope::RequestContext) -> Self {
        Self {
            source: ProtocolProfile::OpenAiChat,
            stream: context.stream.clone(),
            chat_options: context.stream_options.value().cloned(),
            responses_options: None,
            cache: context.cache.clone(),
            execution: envelope::ExecutionHints::default(),
            n: context.n.clone(),
            extensions: CustomSections::default(),
        }
    }

    fn from_responses(context: &envelope::RequestContext) -> Self {
        Self {
            source: ProtocolProfile::OpenAiResponses,
            stream: context.delivery.stream.clone(),
            chat_options: None,
            responses_options: context.delivery.options.value().cloned(),
            cache: context.execution.cache.clone(),
            execution: context.execution.clone(),
            n: Presence::Absent,
            extensions: context.extensions.clone(),
        }
    }
}

/// Projected upstream context: model identity bound at compile time, hints only
/// where the endpoint can represent them. Named conversions: cache-affinity
/// hints on endpoints without `cache_hints` use `OmitSemanticallyInactive`;
/// cross-profile stream options stay absent instead of being remapped.
struct ProjectedContext {
    model: String,
    stream: Presence<bool>,
    chat_options: Option<StreamOptions>,
    responses_options: Option<envelope::StreamOptions>,
    cache: envelope::CacheHints,
    n: Presence<u64>,
}

fn project_context(
    ctx: &WireContext,
    endpoint: &Endpoint,
) -> Result<ProjectedContext, AttemptError> {
    // Provider-scoped extension carriers are never forwarded upstream: they are
    // downstream-origin scoped, and forwarding would be opaque passthrough.
    if ctx.extensions != CustomSections::default() {
        return Err(RepresentationError::UnmigratedSemantic.into());
    }
    // Standard context fields without an admitted wire rule are rejected, not dropped.
    let execution = &ctx.execution;
    if execution.metadata.value().is_some()
        || execution.service_tier.value().is_some()
        || execution.max_tool_calls.value().is_some()
    {
        return Err(RepresentationError::UnmigratedSemantic.into());
    }
    let cache = if endpoint.representation.cache_hints {
        ctx.cache.clone()
    } else {
        envelope::CacheHints::default()
    };
    if ctx.stream == Presence::Value(true) && !endpoint.execution.streaming {
        return Err(AttemptError::Delivery("endpoint cannot stream"));
    }
    let same_profile = ctx.source == endpoint.protocol;
    Ok(ProjectedContext {
        model: endpoint.upstream_model.clone(),
        stream: ctx.stream.clone(),
        chat_options: if same_profile {
            ctx.chat_options.clone()
        } else {
            None
        },
        responses_options: if same_profile {
            ctx.responses_options.clone()
        } else {
            None
        },
        cache,
        n: if same_profile {
            ctx.n.clone()
        } else {
            Presence::Absent
        },
    })
}

fn encode_upstream(
    endpoint: &Endpoint,
    task: &DecodedRequest,
    projected: ProjectedContext,
) -> Result<Vec<u8>, AttemptError> {
    let target = lower_request(
        &task.semantic,
        &task.fidelity,
        codec_profile(endpoint.protocol),
        endpoint.representation.clone(),
    )?;
    let value: Value = match endpoint.protocol {
        ProtocolProfile::OpenAiChat => chat_envelope::encode_request(
            &target,
            &chat_envelope::RequestContext {
                model: projected.model,
                n: projected.n,
                stream: projected.stream,
                stream_options: presence(projected.chat_options),
                cache: projected.cache,
            },
        )?,
        ProtocolProfile::OpenAiResponses => envelope::encode_request(
            &target,
            &envelope::RequestContext {
                model: projected.model,
                delivery: envelope::Delivery {
                    stream: projected.stream,
                    options: presence(projected.responses_options),
                },
                execution: envelope::ExecutionHints {
                    cache: projected.cache,
                    ..envelope::ExecutionHints::default()
                },
                extensions: CustomSections::default(),
            },
        )?,
    };
    let body =
        serde_json::to_vec(&value).map_err(|_| AttemptError::Protocol("body serialization"))?;
    if body.len() > endpoint.execution.request_body_limit {
        return Err(AttemptError::Limit);
    }
    Ok(body)
}

fn assemble(
    endpoint: &Endpoint,
    provider: &ProviderDefinition,
    secret: &SecretMaterial,
    task: &DecodedRequest,
    ctx: &WireContext,
) -> Result<UpstreamRequest, AttemptError> {
    if provider.id != endpoint.provider {
        return Err(AttemptError::Protocol("provider/endpoint mismatch"));
    }
    let projected = project_context(ctx, endpoint)?;
    let streaming = projected.stream == Presence::Value(true);
    let body = encode_upstream(endpoint, task, projected)?;
    Ok(UpstreamRequest {
        origin: endpoint.target.origin.as_str().into(),
        method: "POST",
        path: endpoint.target.path.as_str().into(),
        safe_headers: vec![
            ("content-type".into(), "application/json".into()),
            (
                "accept".into(),
                if streaming {
                    "text/event-stream".into()
                } else {
                    "application/json".into()
                },
            ),
        ],
        auth_header: provider.auth.auth_header(secret),
        body,
    })
}

/// Prepare one upstream attempt from a downstream Chat envelope.
pub fn prepare_chat(
    endpoint: &Endpoint,
    provider: &ProviderDefinition,
    secret: &SecretMaterial,
    request: &chat_envelope::DecodedChatRequest,
) -> Result<UpstreamRequest, AttemptError> {
    assemble(
        endpoint,
        provider,
        secret,
        &request.task,
        &WireContext::from_chat(&request.context),
    )
}

/// Prepare one upstream attempt from a downstream Responses envelope.
pub fn prepare_responses(
    endpoint: &Endpoint,
    provider: &ProviderDefinition,
    secret: &SecretMaterial,
    request: &envelope::DecodedResponsesRequest,
) -> Result<UpstreamRequest, AttemptError> {
    assemble(
        endpoint,
        provider,
        secret,
        &request.task,
        &WireContext::from_responses(&request.context),
    )
}

/// Rewrite upstream-reported response identity to the public model label at the
/// delivery boundary; clients never see upstream binding locators.
fn downstream_metadata(decoded: &DecodedResponse, public_label: &str) -> ResponseMetadata {
    ResponseMetadata {
        id: decoded.metadata.id.clone(),
        model: public_label.into(),
        created: decoded.metadata.created.clone(),
        context: decoded.metadata.context.clone(),
    }
}

enum Intake {
    Pending,
    Static { body: Vec<u8> },
    Sse(ResponsesSseDecoder),
    ChatSse(ChatSseDecoder),
    Rejected,
}

/// One attempt chain: upstream response intake plus downstream delivery.
///
/// Upstream semantic events are buffered so delivery renders from validated IR;
/// the lifecycle commits only when downstream bytes exist, and no retry or
/// fallback may cross that boundary.
pub struct Attempt {
    protocol: ProtocolProfile,
    body_limit: usize,
    limits: SseLimits,
    replay_origin: Option<crate::semantic::value::ReplayOrigin>,
    intake: Intake,
    lifecycle: Lifecycle,
    events: Vec<StreamEvent>,
    decoded: Option<DecodedResponse>,
}

impl Attempt {
    pub fn new(
        protocol: ProtocolProfile,
        body_limit: usize,
        limits: SseLimits,
        replay_origin: Option<crate::semantic::value::ReplayOrigin>,
    ) -> Self {
        Self {
            protocol,
            body_limit,
            limits,
            replay_origin,
            intake: Intake::Pending,
            lifecycle: Lifecycle::new(),
            events: vec![],
            decoded: None,
        }
    }

    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }

    pub fn events(&self) -> &[StreamEvent] {
        &self.events
    }

    /// Feed the response head. Non-2xx statuses fail classified before any body
    /// decoding; success selects the static or SSE intake by media type.
    pub fn begin(&mut self, status: u16, content_type: &str) -> Result<(), AttemptError> {
        if !matches!(self.intake, Intake::Pending) {
            return Err(AttemptError::Protocol("attempt already began"));
        }
        if let StatusClass::Failure(class) = classify_status(status) {
            self.intake = Intake::Rejected;
            return Err(AttemptError::Status { status, class });
        }
        let media = content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        self.intake = if media == "text/event-stream" {
            match self.protocol {
                ProtocolProfile::OpenAiResponses => Intake::Sse(ResponsesSseDecoder::new(
                    status,
                    content_type,
                    self.limits,
                    self.replay_origin.clone(),
                )?),
                ProtocolProfile::OpenAiChat => {
                    Intake::ChatSse(ChatSseDecoder::new(status, content_type, self.limits)?)
                }
            }
        } else if media == "application/json" || media.ends_with("+json") {
            Intake::Static { body: vec![] }
        } else {
            return Err(AttemptError::Protocol("unexpected response media type"));
        };
        Ok(())
    }

    /// Consume body bytes; returns the consumed length and decoded semantic
    /// events. Unconsumed suffixes stay with the caller (backpressure).
    pub fn push(&mut self, chunk: &[u8]) -> Result<(usize, Vec<StreamEvent>), AttemptError> {
        let result = self.push_inner(chunk);
        if result.is_err() {
            self.intake = Intake::Rejected;
        }
        result
    }

    fn push_inner(&mut self, chunk: &[u8]) -> Result<(usize, Vec<StreamEvent>), AttemptError> {
        match &mut self.intake {
            Intake::Static { body } => {
                if body.len().saturating_add(chunk.len()) > self.body_limit {
                    return Err(AttemptError::Limit);
                }
                body.extend_from_slice(chunk);
                Ok((chunk.len(), vec![]))
            }
            Intake::Sse(decoder) => {
                let mut consumed = 0;
                let mut events = vec![];
                while consumed < chunk.len() {
                    let (n, decoded) = decoder.consume(&chunk[consumed..])?;
                    let start = self.events.len();
                    self.events.extend(decoded);
                    events.extend_from_slice(&self.events[start..]);
                    consumed += n;
                    if n == 0 {
                        break;
                    }
                }
                Ok((consumed, events))
            }
            Intake::ChatSse(decoder) => {
                let mut consumed = 0;
                let mut events = vec![];
                while consumed < chunk.len() {
                    let (n, decoded) = decoder.consume(&chunk[consumed..])?;
                    let start = self.events.len();
                    self.events.extend(decoded);
                    events.extend_from_slice(&self.events[start..]);
                    consumed += n;
                    if n == 0 {
                        break;
                    }
                }
                Ok((consumed, events))
            }
            _ => Err(AttemptError::Protocol("attempt is closed")),
        }
    }

    /// Validate the complete upstream terminal and materialize semantic output.
    pub fn finish(&mut self) -> Result<&DecodedResponse, AttemptError> {
        if self.decoded.is_none() {
            let decoded = match &mut self.intake {
                Intake::Static { body } => match self.protocol {
                    ProtocolProfile::OpenAiChat => chat_envelope::decode_response_bytes(body)?,
                    ProtocolProfile::OpenAiResponses => envelope::decode_response_bytes(body)?,
                },
                Intake::Sse(decoder) => {
                    decoder.finish()?;
                    decoder.materialize()?
                }
                Intake::ChatSse(decoder) => {
                    decoder.finish()?;
                    decoder.materialize()?
                }
                _ => return Err(AttemptError::Protocol("attempt is closed")),
            };
            // Trusted execution boundary: static decode leaves replay records
            // unbound; the attempt binds its endpoint's source scope (ADR 0008).
            let mut decoded = decoded;
            if let Some(origin) = &self.replay_origin {
                decoded.fidelity.bind_replay_origin(origin)?;
            }
            self.intake = Intake::Rejected;
            self.decoded = Some(decoded);
        }
        match &self.decoded {
            Some(decoded) => Ok(decoded),
            None => Err(AttemptError::Protocol("attempt not finished")),
        }
    }

    /// Deliver the attempt as one downstream JSON body. This is the commit point.
    pub fn render_json(
        &mut self,
        downstream: ProtocolProfile,
        target: &GenerationRepresentationContract,
        public_label: &str,
    ) -> Result<Vec<u8>, AttemptError> {
        let decoded = self
            .decoded
            .as_ref()
            .ok_or(AttemptError::Protocol("attempt not finished"))?;
        let metadata = downstream_metadata(decoded, public_label);
        // The delivery contract owns the target's replay scope and fact policy;
        // the attempt's own scope only bound intake.
        let representation = lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &metadata,
            codec_profile(downstream),
            target.clone(),
        )?;
        let value = match downstream {
            ProtocolProfile::OpenAiChat => chat_envelope::encode_response(&representation)?,
            ProtocolProfile::OpenAiResponses => envelope::encode_response(&representation)?,
        };
        let body =
            serde_json::to_vec(&value).map_err(|_| AttemptError::Protocol("body serialization"))?;
        self.lifecycle.commit()?;
        self.lifecycle.terminal()?;
        Ok(body)
    }

    /// Deliver the buffered semantic events as one downstream SSE byte stream.
    /// This is the commit point; callers own framing transport and cancellation.
    pub fn render_stream(
        &mut self,
        downstream: ProtocolProfile,
        target: &GenerationRepresentationContract,
        public_label: &str,
        limits: SseLimits,
        chat_options: StreamOptions,
        padding: Obfuscation,
    ) -> Result<Vec<bytes::Bytes>, AttemptError> {
        let decoded = self
            .decoded
            .as_ref()
            .ok_or(AttemptError::Protocol("attempt not finished"))?;
        let metadata = downstream_metadata(decoded, public_label);
        // Event rendering does not pass through `lower_response`, so the
        // response-level delivery policy is checked here, before any frame.
        crate::lowering::generation::require_reported_facts(&decoded.semantic, &metadata, target)?;
        let contract = target.clone();
        let mut frames = vec![];
        match downstream {
            ProtocolProfile::OpenAiResponses => {
                let mut encoder = ResponsesSseEncoder::new(metadata, contract, limits, padding)?;
                for event in &self.events {
                    frames.extend(encoder.encode(event, &decoded.fidelity)?);
                }
                encoder.finish()?;
            }
            ProtocolProfile::OpenAiChat => {
                let mut encoder =
                    ChatSseEncoder::new(metadata, contract, limits, chat_options, padding)?;
                for event in &self.events {
                    frames.extend(encoder.encode(event, &decoded.fidelity)?);
                }
                encoder.finish()?;
            }
        }
        self.lifecycle.commit()?;
        self.lifecycle.terminal()?;
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider::{SecretMaterial, catalog},
        topology::catalog::{deepseek_flash, default_topology, mimo_v2_6_pro, xiaomi_endpoints},
    };
    use serde_json::json;

    fn secret() -> SecretMaterial {
        SecretMaterial::new("sk-test-0001").unwrap()
    }

    fn chat_request(body: Value) -> chat_envelope::DecodedChatRequest {
        chat_envelope::decode_request_bytes(body.to_string().as_bytes()).unwrap()
    }

    fn responses_request(body: Value) -> envelope::DecodedResponsesRequest {
        envelope::decode_request_bytes(body.to_string().as_bytes()).unwrap()
    }

    fn endpoint(id: &str) -> Endpoint {
        default_topology()
            .unwrap()
            .endpoint(&crate::topology::EndpointId::new(id).unwrap())
            .unwrap()
            .clone()
    }

    #[test]
    fn upstream_requests_bind_model_identity_and_fixed_paths() {
        let request = chat_request(json!({
            "model": "deepseek-flash",
            "messages": [{"role": "user", "content": "hello"}]
        }));
        let upstream = prepare_chat(
            &endpoint("deepseek-chat"),
            &catalog::deepseek(),
            &secret(),
            &request,
        )
        .unwrap();
        assert_eq!(upstream.origin, "https://api.deepseek.com");
        assert_eq!(upstream.path, "/chat/completions");
        assert_eq!(upstream.method, "POST");
        assert_eq!(
            upstream.auth_header,
            (
                "authorization".to_string(),
                "Bearer sk-test-0001".to_string()
            )
        );
        let body: Value = serde_json::from_slice(&upstream.body).unwrap();
        assert_eq!(body["model"], "deepseek-flash");
        assert_eq!(body["messages"][0]["content"], "hello");

        let request = responses_request(json!({
            "model": "mimo-v2.6-pro",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "hello"}]}]
        }));
        let upstream = prepare_responses(
            &endpoint("xiaomi-responses"),
            &catalog::xiaomi(),
            &secret(),
            &request,
        )
        .unwrap();
        assert_eq!(upstream.origin, "https://api.xiaomimimo.com");
        assert_eq!(upstream.path, "/v1/responses");
        let body: Value = serde_json::from_slice(&upstream.body).unwrap();
        assert_eq!(body["model"], "mimo-v2.6-pro");
        assert_eq!(body["store"], false);
    }

    #[test]
    fn cache_hints_are_omitted_where_undeclared_not_rewritten() {
        let request = responses_request(json!({
            "model": "mimo-v2.6-pro",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "hello"}]}],
            "prompt_cache_key": "session-0001"
        }));
        let upstream = prepare_responses(
            &endpoint("xiaomi-responses"),
            &catalog::xiaomi(),
            &secret(),
            &request,
        )
        .unwrap();
        let body: Value = serde_json::from_slice(&upstream.body).unwrap();
        assert!(
            body.get("prompt_cache_key").is_none(),
            "hints are inactive on this entry"
        );
    }

    #[test]
    fn undeclared_standard_context_and_extensions_are_rejected_not_dropped() {
        let request = responses_request(json!({
            "model": "deepseek-flash",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "hello"}]}],
            "service_tier": "auto"
        }));
        assert!(matches!(
            prepare_responses(
                &endpoint("deepseek-responses"),
                &catalog::deepseek(),
                &secret(),
                &request,
            ),
            Err(AttemptError::Representation(
                RepresentationError::UnmigratedSemantic
            ))
        ));
        let request = responses_request(json!({
            "model": "deepseek-flash",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "hello"}]}],
            "client_metadata": {"trace": "opaque"}
        }));
        assert!(matches!(
            prepare_responses(
                &endpoint("deepseek-responses"),
                &catalog::deepseek(),
                &secret(),
                &request,
            ),
            Err(AttemptError::Representation(
                RepresentationError::UnmigratedSemantic
            ))
        ));
    }

    #[test]
    fn strict_tools_are_rejected_on_the_standard_target() {
        let request = chat_request(json!({
            "model": "deepseek-flash",
            "messages": [{"role": "user", "content": "hello"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "lookup",
                    "description": "lookup",
                    "parameters": {
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                        "required": ["city"],
                        "additionalProperties": false
                    },
                    "strict": true
                }
            }]
        }));
        assert!(matches!(
            prepare_chat(
                &endpoint("deepseek-chat"),
                &catalog::deepseek(),
                &secret(),
                &request
            ),
            Err(AttemptError::Representation(
                RepresentationError::StrictTools
            ))
        ));
    }

    #[test]
    fn streaming_requires_an_endpoint_that_can_stream() {
        let request = chat_request(json!({
            "model": "deepseek-flash",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": true
        }));
        let mut endpoint = endpoint("deepseek-chat");
        endpoint.execution.streaming = false;
        assert!(matches!(
            prepare_chat(&endpoint, &catalog::deepseek(), &secret(), &request),
            Err(AttemptError::Delivery(_))
        ));
    }

    #[test]
    fn entry_admission_uses_the_public_contract_not_the_endpoint() {
        let request = chat_request(json!({
            "model": "deepseek-flash",
            "messages": [{"role": "user", "content": "hello"}],
            "temperature": 0.5
        }));
        let mut model = deepseek_flash();
        model.contract.temperature = false;
        assert!(matches!(
            admit(&model, &request.task),
            Err(AttemptError::Representation(
                RepresentationError::Temperature
            ))
        ));
        assert!(admit(&deepseek_flash(), &request.task).is_ok());
    }

    #[test]
    fn catalog_models_cover_both_providers() {
        assert_eq!(deepseek_flash().id.as_str(), "deepseek-flash");
        assert_eq!(mimo_v2_6_pro().id.as_str(), "mimo-v2.6-pro");
        assert_eq!(xiaomi_endpoints().len(), 2);
    }
}
