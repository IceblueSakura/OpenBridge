//! Trusted session for administrative probes against a fixed Upstream Target.
//!
//! The session uses only the endpoint, model, adapter, and startup credential snapshot from the
//! compiled registry. A network or protocol failure produces a conservative outcome for that
//! probe and does not block other probes in the same report.

use axum::body::to_bytes;
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;

use crate::{
    core::{ApiProtocol, ApiRequest, EmbeddingRequest, OperationKind},
    credential::CredentialStore,
    oauth2_credentials::OAuth2CredentialManager,
    provider::{
        PreparedUpstreamRequest, ProviderAdapter, ProviderKind, ProviderRequestContext,
        StreamEventStatus,
    },
    registry::{RuntimeRegistry, UpstreamApi, UpstreamTarget},
    transport::{
        sse::SseDecoder,
        upstream::{UpstreamResponse, UpstreamTransport},
    },
};

use super::{
    ModelListProbeResult, ProbeError, ProbeOptions, ProbeResult, SupportStatus, TargetProbeReport,
    payload::{
        GenerationProbeMode, is_embedding_response, is_protocol_response, probe_embedding_request,
        probe_text_request,
    },
};

const PROBE_MAX_OUTPUT_TOKENS: u32 = 16;

/// Runs the selected probes with the same trusted configuration as the data plane.
///
/// This function accesses only the fixed endpoint for `upstream_target_id`. It accepts no external
/// URL, model, or header parameters, so diagnostics cannot expand SSRF or credential use.
pub async fn probe_upstream_target(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    // Resolve the target and reject disabled registrations before credential access or egress.
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or_else(|| ProbeError::UnknownUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        })?;
    if !target.enabled() {
        return Err(ProbeError::DisabledUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        });
    }

    // Run the common API-key probe without local client identity or environment state.
    run_probe_session(registry, target, transport, credentials, selection).await
}

/// Probes a ChatGPT target by borrowing one guarded OAuth2 access-token lease.
///
/// The manager owns refresh and account binding. This entry point accepts no endpoint or
/// credential override, and it deliberately supports only the ChatGPT Provider boundary.
pub async fn probe_upstream_target_with_oauth2(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    oauth2_credentials: &OAuth2CredentialManager,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    // Resolve the target and reject disabled registrations before OAuth2 manager access or egress.
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or_else(|| ProbeError::UnknownUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        })?;
    if !target.enabled() {
        return Err(ProbeError::DisabledUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        });
    }
    if target.kind() != ProviderKind::ChatGpt {
        return Err(ProbeError::OAuth2UnsupportedTarget {
            upstream_target: upstream_target_id.to_owned(),
        });
    }

    // Borrow the current account-bound generation and enforce compile-time pool affinity.
    let lease = oauth2_credentials
        .lease_for_request(target.kind())
        .await
        .map_err(|_| ProbeError::CredentialUnavailable)?;
    if lease.pool_id() != target.credential_pool_id() {
        return Err(ProbeError::CredentialUnavailable);
    }
    let credential = lease
        .credential()
        .map_err(|_| ProbeError::CredentialUnavailable)?;

    // Prepare the fixed ChatGPT authentication headers before entering the common probe session.
    let adapter = ProviderAdapter::for_kind(target.kind());
    let headers = adapter
        .build_outbound_headers(&credential, &HeaderMap::new())
        .map_err(|_| ProbeError::AuthenticationPreparation)?;
    run_probe_session_with_headers(registry, target, transport, headers, selection).await
}

/// Resolves one credential and runs the common probe sequence against an already trusted target.
async fn run_probe_session(
    registry: &RuntimeRegistry,
    target: &UpstreamTarget,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    // Select the first member of the target's compile-time credential pool deterministically.
    let pool = registry
        .credential_pool(target.credential_pool_id())
        .ok_or(ProbeError::CredentialUnavailable)?;
    let credential = credentials
        .upstream_pool(target.kind(), pool.id(), pool.kind())
        .map_err(|_| ProbeError::CredentialUnavailable)?;
    let credential = credential
        .into_iter()
        .next()
        .ok_or(ProbeError::CredentialUnavailable)?;

    // Select the compile-time adapter and prepare the sensitive outbound headers required by probes.
    let adapter = ProviderAdapter::for_kind(target.kind());
    let headers = adapter
        .build_outbound_headers(&credential, &HeaderMap::new())
        .map_err(|_| ProbeError::AuthenticationPreparation)?;
    run_probe_session_with_headers(registry, target, transport, headers, selection).await
}

/// Runs the fixed probe sequence with already prepared, purpose-bound outbound headers.
async fn run_probe_session_with_headers(
    registry: &RuntimeRegistry,
    target: &UpstreamTarget,
    transport: &dyn UpstreamTransport,
    headers: HeaderMap,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    let session = ProbeSession {
        target,
        transport,
        adapter: ProviderAdapter::for_kind(target.kind()),
        chatgpt_instructions: registry.chatgpt_instructions(),
        headers,
        max_response_bytes: registry.limits().max_json_response_body_bytes(),
        max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
    };

    // Run each probe independently so one failure affects only its outcome.
    let list_models = if selection.list_models {
        Some(session.probe_list_models().await)
    } else {
        None
    };
    let chat = if selection.chat {
        Some(session.probe_text(ApiProtocol::ChatCompletions).await)
    } else {
        None
    };
    let responses = if selection.responses {
        Some(session.probe_text(ApiProtocol::Responses).await)
    } else {
        None
    };
    let embeddings = if selection.embeddings {
        Some(session.probe_embeddings().await)
    } else {
        None
    };

    // Assemble a structured report without credentials, request bodies, or response bodies.
    Ok(TargetProbeReport {
        upstream_target_id: target.id().to_owned(),
        list_models,
        chat,
        responses,
        embeddings,
    })
}

struct ProbeSession<'a> {
    target: &'a UpstreamTarget,
    transport: &'a dyn UpstreamTransport,
    adapter: ProviderAdapter,
    chatgpt_instructions: Option<&'a str>,
    headers: HeaderMap,
    max_response_bytes: usize,
    max_sse_event_bytes: usize,
}

impl ProbeSession<'_> {
    /// Queries the fixed model-list endpoint and extracts visible model IDs.
    async fn probe_list_models(&self) -> ModelListProbeResult {
        // Send the fixed model-list request and extract model IDs.
        let request = match self.adapter.prepare_model_list_request() {
            Ok(request) => request,
            Err(_) => {
                return ModelListProbeResult {
                    outcome: ProbeResult::unknown(None),
                    configured_model_listed: None,
                    model_ids: Vec::new(),
                };
            }
        };
        match self.send_json(request).await {
            Ok(response) => {
                let Some(model_ids) = self.adapter.model_list_ids(&response.body) else {
                    return ModelListProbeResult {
                        outcome: ProbeResult::unknown(Some(response.status)),
                        configured_model_listed: None,
                        model_ids: Vec::new(),
                    };
                };
                let configured_model_listed =
                    Some(self.target.upstream_apis().any(|(_, upstream_api)| {
                        model_ids
                            .iter()
                            .any(|model| model == upstream_api.upstream_model())
                    }));
                ModelListProbeResult {
                    outcome: ProbeResult::supported(response.status),
                    configured_model_listed,
                    model_ids,
                }
            }
            Err(outcome) => ModelListProbeResult {
                outcome,
                configured_model_listed: None,
                model_ids: Vec::new(),
            },
        }
    }

    /// Executes the target protocol's fixed minimum text request.
    async fn probe_text(&self, protocol: ApiProtocol) -> ProbeResult {
        // Resolve the target API and select the wire mode required by the fixed Provider profile.
        let Some(upstream_api) = self.target.upstream_api(protocol.operation()) else {
            return ProbeResult {
                state: SupportStatus::Unsupported,
                http_status: None,
            };
        };
        let mode =
            if self.target.kind() == ProviderKind::ChatGpt && protocol == ApiProtocol::Responses {
                GenerationProbeMode::Sse
            } else {
                GenerationProbeMode::Json
            };
        let request = probe_text_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
            mode,
        );

        // Report support only when JSON has the expected shape or SSE reaches a recognized terminal.
        match mode {
            GenerationProbeMode::Json => match self.send_protocol_json(protocol, request).await {
                Ok(response) if is_protocol_response(protocol, &response.body) => {
                    ProbeResult::supported(response.status)
                }
                Ok(response) => ProbeResult::unknown(Some(response.status)),
                Err(outcome) => outcome,
            },
            GenerationProbeMode::Sse => self.send_protocol_sse(protocol, request).await,
        }
    }

    /// Executes one fixed single-text Embeddings Create request.
    async fn probe_embeddings(&self) -> ProbeResult {
        // Resolve the registered Embeddings API and build its fixed request body.
        let Some(upstream_api) = self.target.upstream_api(OperationKind::EmbeddingsCreate) else {
            return ProbeResult {
                state: SupportStatus::Unsupported,
                http_status: None,
            };
        };
        let body = probe_embedding_request(upstream_api.upstream_model());
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = EmbeddingRequest::new(Bytes::from(body));

        // Bind the request through the fixed adapter and require a recognizable Embeddings response.
        let request = match self
            .adapter
            .prepare_embedding_routed_request(&request, upstream_api)
        {
            Ok(request) => request,
            Err(_) => return ProbeResult::unknown(None),
        };
        match self.send_json(request).await {
            Ok(response)
                if is_embedding_response(&response.body, upstream_api.upstream_model()) =>
            {
                ProbeResult::supported(response.status)
            }
            Ok(response) => ProbeResult::unknown(Some(response.status)),
            Err(outcome) => outcome,
        }
    }

    /// Binds the protocol request through the compile-time adapter and sends it over trusted transport.
    async fn send_protocol_json(
        &self,
        protocol: ApiProtocol,
        body: Value,
    ) -> Result<JsonResponse, ProbeResult> {
        // Bind the fixed payload through the target's registered API and Provider adapter.
        let request = self.prepare_protocol_request(protocol, body)?;

        // Send through trusted transport and decode the response under the shared body limit.
        self.send_json(request).await
    }

    /// Binds and consumes one protocol request as bounded SSE until a recognized terminal event.
    async fn send_protocol_sse(&self, protocol: ApiProtocol, body: Value) -> ProbeResult {
        // Bind the fixed payload through the target's registered API and Provider adapter.
        let request = match self.prepare_protocol_request(protocol, body) {
            Ok(request) => request,
            Err(outcome) => return outcome,
        };

        // Send the request and validate status and media type before consuming stream bytes.
        let response = match self
            .transport
            .send(self.target, request, self.headers.clone())
            .await
        {
            Ok(response) => response,
            Err(_) => return ProbeResult::unknown(None),
        };
        let status = response.status();
        if !status.is_success() {
            return ProbeResult::from_http_status(status);
        }
        if !is_event_stream(response.headers()) {
            return ProbeResult::unknown(Some(status));
        }

        // Decode bounded chunks and stop as soon as the adapter recognizes a terminal event.
        let mut total_bytes = 0usize;
        let mut decoder = SseDecoder::new(self.max_sse_event_bytes);
        let mut stream = response.into_body().into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(_) => return ProbeResult::unknown(Some(status)),
            };
            total_bytes = match total_bytes.checked_add(chunk.len()) {
                Some(total) if total <= self.max_response_bytes => total,
                _ => return ProbeResult::unknown(Some(status)),
            };
            let events = match decoder.push(&chunk) {
                Ok(events) => events,
                Err(_) => return ProbeResult::unknown(Some(status)),
            };
            if let Some(outcome) = self.classify_sse_events(protocol, status, events) {
                return outcome;
            }
        }

        // Finalize an unterminated last event and require an explicit normal terminal.
        let events = match decoder.finish() {
            Ok(events) => events,
            Err(_) => return ProbeResult::unknown(Some(status)),
        };
        self.classify_sse_events(protocol, status, events)
            .unwrap_or_else(|| ProbeResult::unknown(Some(status)))
    }

    /// Classifies framed SSE events and returns a conclusion only for a terminal event.
    fn classify_sse_events(
        &self,
        protocol: ApiProtocol,
        status: StatusCode,
        events: Vec<crate::transport::sse::SseEvent>,
    ) -> Option<ProbeResult> {
        // Delegate lifecycle semantics to the Provider adapter and stop at the first terminal.
        for event in events {
            let event = self.adapter.classify_sse_event(protocol, event).ok()?;
            match event.status() {
                StreamEventStatus::Continue => {}
                StreamEventStatus::Completed => return Some(ProbeResult::supported(status)),
                StreamEventStatus::Failed => return Some(ProbeResult::unknown(Some(status))),
            }
        }
        None
    }

    /// Serializes and binds one generation request through its registered Upstream API.
    fn prepare_protocol_request(
        &self,
        protocol: ApiProtocol,
        body: Value,
    ) -> Result<PreparedUpstreamRequest, ProbeResult> {
        // Serialize the fixed body and resolve the already validated Upstream API registration.
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = ApiRequest::new(protocol, Bytes::from(body));
        let upstream_api = self
            .target
            .upstream_api(protocol.operation())
            .expect("probe protocol has a configured upstream API");

        // Let the compile-time adapter bind the model, wire mappings, and relative path.
        self.adapter
            .prepare_routed_request(
                &request,
                upstream_api,
                ProviderRequestContext::new(self.chatgpt_instructions),
            )
            .map_err(|_| ProbeResult::unknown(None))
    }

    /// Sends a prepared request and normalizes transport, HTTP, and JSON failures to a conservative outcome.
    async fn send_json(
        &self,
        request: crate::provider::PreparedUpstreamRequest,
    ) -> Result<JsonResponse, ProbeResult> {
        // Send the request and convert transport failures to a conservative unknown outcome.
        let response = self
            .transport
            .send(self.target, request, self.headers.clone())
            .await
            .map_err(|_| ProbeResult::unknown(None))?;

        // Validate the status and JSON body before deriving a basic observation.
        decode_json_response(response, self.max_response_bytes).await
    }

    /// Restricts probe output tokens to the intersection of the model declaration and the fixed safety limit.
    fn probe_max_output_tokens(&self, upstream_api: &UpstreamApi) -> u32 {
        upstream_api
            .model()
            .context_length()
            .output_tokens()
            .unwrap_or(PROBE_MAX_OUTPUT_TOKENS)
            .min(PROBE_MAX_OUTPUT_TOKENS)
    }
}

/// Returns whether the response declares the SSE media type, allowing optional parameters.
fn is_event_stream(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("text/event-stream"))
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
}

/// Reads a successful JSON response within the fixed limit and classifies HTTP failures first.
async fn decode_json_response(
    response: UpstreamResponse,
    max_response_bytes: usize,
) -> Result<JsonResponse, ProbeResult> {
    // Read the response body within the configured limit and classify HTTP failures first.
    let status = response.status();
    let body = to_bytes(response.into_body(), max_response_bytes)
        .await
        .map_err(|_| ProbeResult::unknown(Some(status)))?;
    if !status.is_success() {
        return Err(ProbeResult::from_http_status(status));
    }

    // Accept only valid JSON so an error page cannot be reported as protocol success.
    let body = serde_json::from_slice(&body).map_err(|_| ProbeResult::unknown(Some(status)))?;
    Ok(JsonResponse { status, body })
}
