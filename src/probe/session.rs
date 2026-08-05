//! Trusted session for capability probes against a fixed Upstream Target.
//!
//! The session uses only the endpoint, model, adapter, and startup credential snapshot from the
//! compiled registry. A network or protocol failure produces a conservative outcome for that
//! probe and does not block other probes in the same report.

use axum::body::to_bytes;
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, HeaderValue, StatusCode, header::ACCEPT};
use serde_json::Value;

use crate::{
    codex_identity::CodexRequestIdentity,
    core::{ApiProtocol, ApiRequest},
    credential::CredentialStore,
    provider::{ProviderAdapter, ProviderKind, StreamEventStatus},
    registry::{RuntimeRegistry, UpstreamApi, UpstreamTarget},
    transport::sse::{SseDecoder, SseEvent},
    transport::upstream::{UpstreamResponse, UpstreamTransport},
};

use super::{
    CodexCompatibilityReport, ModelListProbeResult, ProbeError, ProbeOptions, ProbeResult,
    SupportStatus, TargetProbeReport, ToolCallProbeResult,
    payload::{
        codex_responses_text_request, is_protocol_response, probe_text_request, probe_tool_request,
        tool_result_replay_request,
    },
};

const PROBE_MAX_OUTPUT_TOKENS: u32 = 16;

/// Runs the selected probes with the same trusted configuration as the data plane.
///
/// This function accesses only the fixed endpoint for `upstream_target_id`. It accepts no external
/// URL, model, or header parameters, so diagnostic capabilities cannot expand SSRF or credential use.
pub async fn probe_upstream_target(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    // Resolve the target and keep the disabled ChatGPT path behind its identity-bound entry point.
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or_else(|| ProbeError::UnknownUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        })?;
    if target.kind() == ProviderKind::ChatGpt {
        return Err(ProbeError::CodexIdentityRequired);
    }

    // Run the ordinary API-key probe without any Codex-specific identity context.
    run_probe_session(
        registry,
        target,
        transport,
        credentials,
        selection,
        HeaderMap::new(),
        None,
    )
    .await
}

/// Runs the disabled ChatGPT probe with the source-pinned Codex-compatible request identity.
///
/// This entry point accepts only the compiled ChatGPT target and first-stage model/Responses
/// selections. The identity provides no URL, model, credential, or arbitrary header input.
pub async fn probe_chatgpt_upstream_target(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
    identity: &CodexRequestIdentity,
) -> Result<TargetProbeReport, ProbeError> {
    // Resolve the exact disabled ChatGPT target and reject unsupported first-stage operations.
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or_else(|| ProbeError::UnknownUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        })?;
    if target.kind() != ProviderKind::ChatGpt {
        return Err(ProbeError::CodexIdentityUnexpected);
    }
    if selection.is_empty() || selection.chat || selection.function_calling {
        return Err(ProbeError::InvalidChatGptSelection);
    }

    // Copy the private identity into the trusted Provider hook and retain only redacted report facts.
    run_probe_session(
        registry,
        target,
        transport,
        credentials,
        selection,
        identity.request_headers(),
        Some(identity),
    )
    .await
}

/// Resolves one credential and runs the common probe sequence against an already trusted target.
async fn run_probe_session(
    registry: &RuntimeRegistry,
    target: &UpstreamTarget,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
    identity_headers: HeaderMap,
    codex_identity: Option<&CodexRequestIdentity>,
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
        .build_outbound_headers(&credential, &identity_headers)
        .map_err(|_| ProbeError::AuthenticationPreparation)?;
    let session = ProbeSession {
        target,
        transport,
        adapter,
        headers,
        max_response_bytes: registry.limits().max_json_response_body_bytes(),
        max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
        client_version: codex_identity.map(|identity| identity.version().to_owned()),
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
    let chat_function_calling = if selection.function_calling {
        Some(
            session
                .probe_function_calling(ApiProtocol::ChatCompletions)
                .await,
        )
    } else {
        None
    };
    let responses_function_calling = if selection.function_calling {
        Some(session.probe_function_calling(ApiProtocol::Responses).await)
    } else {
        None
    };

    // Assemble a structured report without credentials, request bodies, or response bodies.
    Ok(TargetProbeReport {
        upstream_target_id: target.id().to_owned(),
        codex_compatibility: codex_identity.map(|identity| CodexCompatibilityReport {
            profile_version: identity.version().to_owned(),
            platform_family: identity.platform_family().to_owned(),
            platform_os: identity.platform_os().to_owned(),
            user_agent_matches_reference_profile: true,
        }),
        list_models,
        chat,
        responses,
        chat_function_calling,
        responses_function_calling,
    })
}

struct ProbeSession<'a> {
    target: &'a UpstreamTarget,
    transport: &'a dyn UpstreamTransport,
    adapter: ProviderAdapter,
    headers: HeaderMap,
    max_response_bytes: usize,
    max_sse_event_bytes: usize,
    client_version: Option<String>,
}

impl ProbeSession<'_> {
    /// Queries the fixed model-list endpoint and extracts visible model IDs.
    async fn probe_list_models(&self) -> ModelListProbeResult {
        // Send the fixed model-list request and extract model IDs.
        let request = match self
            .adapter
            .prepare_model_list_request(self.client_version.as_deref())
        {
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

    /// Executes the target protocol's minimal non-streaming text request.
    async fn probe_text(&self, protocol: ApiProtocol) -> ProbeResult {
        // Resolve the target API for the protocol and build the minimal text request.
        let Some(upstream_api) = self.target.upstream_api(protocol.operation()) else {
            return ProbeResult {
                state: SupportStatus::Unsupported,
                http_status: None,
            };
        };
        if self.target.kind() == ProviderKind::ChatGpt && protocol == ApiProtocol::Responses {
            return self.probe_codex_responses_sse(upstream_api).await;
        }
        let request = probe_text_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
        );

        // Report support only when the response shape matches the target protocol.
        match self.send_protocol_json(protocol, request).await {
            Ok(response) if is_protocol_response(protocol, &response.body) => {
                ProbeResult::supported(response.status)
            }
            Ok(response) => ProbeResult::unknown(Some(response.status)),
            Err(outcome) => outcome,
        }
    }

    /// Executes the fixed ChatGPT Codex Responses request and requires one valid SSE completion.
    async fn probe_codex_responses_sse(&self, upstream_api: &UpstreamApi) -> ProbeResult {
        // Build the current Codex streaming request and bind only the compiled Responses endpoint.
        let body = serde_json::to_vec(&codex_responses_text_request(upstream_api.upstream_model()))
            .expect("Codex probe request JSON is serializable");
        let request = ApiRequest::new(ApiProtocol::Responses, Bytes::from(body));
        let request = match self.adapter.prepare_routed_request(&request, upstream_api) {
            Ok(request) => request,
            Err(_) => return ProbeResult::unknown(None),
        };

        // Stream and classify the response without retaining or returning Provider body content.
        let mut headers = self.headers.clone();
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        let response = match self.transport.send(self.target, request, headers).await {
            Ok(response) => response,
            Err(_) => return ProbeResult::unknown(None),
        };
        decode_responses_sse(
            response,
            self.adapter,
            self.max_response_bytes,
            self.max_sse_event_bytes,
        )
        .await
    }

    /// Executes the two-request function-call and tool-result replay probe.
    async fn probe_function_calling(&self, protocol: ApiProtocol) -> ToolCallProbeResult {
        // Send the function-call request and extract a replayable tool call.
        let Some(upstream_api) = self.target.upstream_api(protocol.operation()) else {
            return ToolCallProbeResult {
                initial_call: ProbeResult {
                    state: SupportStatus::Unsupported,
                    http_status: None,
                },
                result_replay: None,
            };
        };
        let request = probe_tool_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
        );
        let response = match self.send_protocol_json(protocol, request).await {
            Ok(response) => response,
            Err(outcome) => {
                return ToolCallProbeResult {
                    initial_call: outcome,
                    result_replay: None,
                };
            }
        };
        let Some(replay) = tool_result_replay_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
            &response.body,
        ) else {
            return ToolCallProbeResult {
                initial_call: ProbeResult::unknown(Some(response.status)),
                result_replay: None,
            };
        };

        // Replay the tool result and confirm that the second request still returns the target protocol.
        let replay = match self.send_protocol_json(protocol, replay).await {
            Ok(response) if is_protocol_response(protocol, &response.body) => {
                ProbeResult::supported(response.status)
            }
            Ok(response) => ProbeResult::unknown(Some(response.status)),
            Err(outcome) => outcome,
        };
        ToolCallProbeResult {
            initial_call: ProbeResult::supported(response.status),
            result_replay: Some(replay),
        }
    }

    /// Binds the protocol request through the compile-time adapter and sends it over trusted transport.
    async fn send_protocol_json(
        &self,
        protocol: ApiProtocol,
        body: Value,
    ) -> Result<JsonResponse, ProbeResult> {
        // Serialize the probe body and resolve the selected Upstream API for egress preparation.
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = ApiRequest::new(protocol, Bytes::from(body));
        let upstream_api = self
            .target
            .upstream_api(protocol.operation())
            .expect("probe protocol has a configured upstream API");

        // Let the compile-time adapter bind the model, wire mappings, and relative path.
        let request = self
            .adapter
            .prepare_routed_request(&request, upstream_api)
            .expect("compiled provider adapter accepts both probe protocols");

        // Send through trusted transport and decode the response under the shared body limit.
        self.send_json(request).await
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

        // Validate the status and JSON body before deriving a capability conclusion.
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

/// Streams one Responses SSE body under total and per-event limits and requires one terminal.
async fn decode_responses_sse(
    response: UpstreamResponse,
    adapter: ProviderAdapter,
    max_response_bytes: usize,
    max_sse_event_bytes: usize,
) -> ProbeResult {
    // Reject HTTP errors before reading any body content.
    let status = response.status();
    if !status.is_success() {
        return ProbeResult::from_http_status(status);
    }

    // Decode fragmented events regardless of response metadata, matching Codex's framing behavior.
    let mut source = response.into_body().into_data_stream();
    let mut decoder = SseDecoder::new(max_sse_event_bytes);
    let mut total_bytes = 0usize;
    let mut completed = 0usize;
    while let Some(chunk) = source.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(_) => return ProbeResult::unknown(Some(status)),
        };
        total_bytes = match total_bytes.checked_add(chunk.len()) {
            Some(total) if total <= max_response_bytes => total,
            _ => return ProbeResult::unknown(Some(status)),
        };
        let events = match decoder.push(&chunk) {
            Ok(events) => events,
            Err(_) => return ProbeResult::unknown(Some(status)),
        };
        if observe_responses_events(adapter, events, &mut completed).is_err() {
            return ProbeResult::unknown(Some(status));
        }
    }

    // Finalize the decoder and accept only one non-failing `response.completed` terminal.
    let events = match decoder.finish() {
        Ok(events) => events,
        Err(_) => return ProbeResult::unknown(Some(status)),
    };
    if observe_responses_events(adapter, events, &mut completed).is_err() || completed != 1 {
        ProbeResult::unknown(Some(status))
    } else {
        ProbeResult::supported(status)
    }
}

/// Applies Provider terminal classification and rejects failures or duplicate completions.
fn observe_responses_events(
    adapter: ProviderAdapter,
    events: Vec<SseEvent>,
    completed: &mut usize,
) -> Result<(), ()> {
    // Inspect lifecycle metadata only and discard each event immediately after classification.
    for event in events {
        match adapter
            .classify_sse_event(ApiProtocol::Responses, event)
            .map_err(|_| ())?
            .status()
        {
            StreamEventStatus::Continue => {}
            StreamEventStatus::Completed => {
                *completed = completed.checked_add(1).ok_or(())?;
                if *completed > 1 {
                    return Err(());
                }
            }
            StreamEventStatus::Failed => return Err(()),
        }
    }
    Ok(())
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
