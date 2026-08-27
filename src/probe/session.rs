//! Trusted session for administrative probes against a fixed Upstream Target.
//!
//! The session uses only the endpoint, model, adapter, and startup credential snapshot from the
//! compiled registry. A network or protocol failure produces a conservative outcome for that
//! probe and does not block other probes in the same report.

use std::time::Instant;

use axum::body::to_bytes;
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;

use crate::{
    core::{ApiProtocol, ApiRequest, EmbeddingRequest, OperationKind},
    credential::CredentialStore,
    oauth2_credentials::OAuth2CredentialManager,
    pipeline::normalize_probe_generation_request,
    provider::{
        GenerationProviderAdapter, PreparedUpstreamRequest, ProviderAdapter, ProviderKind,
        ProviderOperationAdapter, StreamEventStatus,
    },
    registry::{CanonicalTaskKind, RuntimeRegistry, UpstreamApi, UpstreamApiKey, UpstreamTarget},
    transport::{
        sse::SseDecoder,
        upstream::{UpstreamResponse, UpstreamTransport},
    },
};

use super::{
    GenerationProbeEvidence, GenerationProbeResult, ModelListProbeResult, ProbeError, ProbeFailure,
    ProbeGenerationMode, ProbeOptions, ProbeProtocol, ProbeReasoningEffort, ProbeResult,
    ProbeTerminal, ProbeTokenUsage, TargetProbeReport,
    payload::{
        is_embedding_response, is_protocol_response, probe_embedding_request, probe_text_request,
    },
};

const PROBE_MAX_OUTPUT_TOKENS: u32 = 16;
const MAX_REPORTED_MODEL_IDS: usize = 1_024;

/// Runs the selected probes with the same trusted configuration as the data plane.
///
/// This function accesses only the fixed endpoint for `upstream_target_id`. An optional model ID
/// changes only the built-in synthetic JSON field; URL, path, headers, and credentials stay trusted.
pub async fn probe_upstream_target(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    selection.validate()?;
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
    selection.validate()?;
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
        default_instructions: registry.default_instructions(),
        headers,
        max_response_bytes: registry.limits().max_json_response_body_bytes(),
        max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
    };

    // Run each probe independently so one failure affects only its outcome.
    let list_models = if selection.list_models {
        Some(
            session
                .probe_list_models(selection.upstream_model.as_deref())
                .await,
        )
    } else {
        None
    };
    let mut generation = Vec::new();
    if selection.chat {
        generation.extend(
            session
                .probe_text_matrix(ApiProtocol::ChatCompletions, &selection)
                .await,
        );
    }
    if selection.responses {
        generation.extend(
            session
                .probe_text_matrix(ApiProtocol::Responses, &selection)
                .await,
        );
    }
    let embeddings = if selection.embeddings {
        Some(session.probe_embeddings().await)
    } else {
        None
    };

    // Assemble a structured report without credentials, request bodies, or response bodies.
    Ok(TargetProbeReport {
        upstream_target_id: target.id().to_owned(),
        requested_model: selection.upstream_model,
        allow_unbounded_streaming_output: selection.allow_unbounded_streaming_output,
        list_models,
        generation,
        embeddings,
    })
}

struct ProbeSession<'a> {
    target: &'a UpstreamTarget,
    transport: &'a dyn UpstreamTransport,
    adapter: ProviderAdapter,
    default_instructions: Option<&'a str>,
    headers: HeaderMap,
    max_response_bytes: usize,
    max_sse_event_bytes: usize,
}

impl ProbeSession<'_> {
    /// Queries the fixed model-list endpoint and extracts visible model IDs.
    async fn probe_list_models(&self, requested_model: Option<&str>) -> ModelListProbeResult {
        // Send the fixed model-list request and extract model IDs.
        let request = match self.adapter.prepare_model_list_request() {
            Ok(request) => request,
            Err(_) => {
                return ModelListProbeResult {
                    outcome: ProbeResult::inconclusive(None, ProbeFailure::RequestPreparation),
                    configured_model_listed: None,
                    requested_model_listed: None,
                    model_id_count: None,
                    model_ids_truncated: false,
                    model_ids: Vec::new(),
                };
            }
        };
        match self.send_json(request).await {
            Ok(response) => {
                let Some(model_ids) = self.adapter.model_list_ids(&response.body) else {
                    return ModelListProbeResult {
                        outcome: ProbeResult::inconclusive(
                            Some(response.status),
                            ProbeFailure::UnexpectedResponse,
                        ),
                        configured_model_listed: None,
                        requested_model_listed: None,
                        model_id_count: None,
                        model_ids_truncated: false,
                        model_ids: Vec::new(),
                    };
                };
                let configured_model_listed =
                    Some(self.target.upstream_apis().any(|(_, upstream_api)| {
                        model_ids
                            .iter()
                            .any(|model| model == upstream_api.upstream_model())
                    }));
                let requested_model_listed = requested_model
                    .map(|requested_model| model_ids.iter().any(|model| model == requested_model));
                let model_id_count = model_ids.len();
                let mut reported_model_ids = model_ids;
                reported_model_ids.truncate(MAX_REPORTED_MODEL_IDS);
                ModelListProbeResult {
                    outcome: ProbeResult::accepted(response.status),
                    configured_model_listed,
                    requested_model_listed,
                    model_id_count: Some(model_id_count),
                    model_ids_truncated: model_id_count > reported_model_ids.len(),
                    model_ids: reported_model_ids,
                }
            }
            Err(outcome) => ModelListProbeResult {
                outcome,
                configured_model_listed: None,
                requested_model_listed: None,
                model_id_count: None,
                model_ids_truncated: false,
                model_ids: Vec::new(),
            },
        }
    }

    /// Executes every selected delivery/reasoning case for one Generation protocol.
    async fn probe_text_matrix(
        &self,
        protocol: ApiProtocol,
        selection: &ProbeOptions,
    ) -> Vec<GenerationProbeResult> {
        let mut results = Vec::new();
        for mode in &selection.generation_modes {
            for reasoning_effort in &selection.reasoning_efforts {
                let result = self
                    .probe_text_case(
                        protocol,
                        *mode,
                        *reasoning_effort,
                        selection.upstream_model.as_deref(),
                        selection.allow_unbounded_streaming_output,
                    )
                    .await;
                results.push(result);
            }
        }
        results
    }

    /// Executes one fixed synthetic Generation request without registered model-specific rewrites.
    async fn probe_text_case(
        &self,
        protocol: ApiProtocol,
        mode: ProbeGenerationMode,
        reasoning_effort: ProbeReasoningEffort,
        upstream_model_override: Option<&str>,
        allow_unbounded_streaming_output: bool,
    ) -> GenerationProbeResult {
        let started = Instant::now();
        if self.target.canonical_task() != CanonicalTaskKind::Generation {
            return generation_result(
                protocol,
                mode,
                reasoning_effort,
                upstream_model_override,
                elapsed_millis(&started),
                ProbeResult::unsupported(ProbeFailure::OperationUnavailable),
                None,
            );
        }
        // Use an explicit candidate model, or the registered model for this exact protocol.
        let registered_api = self.target.upstream_api(UpstreamApiKey::new(
            protocol.operation(),
            self.target.canonical_task(),
        ));
        let Some(upstream_model) =
            upstream_model_override.or_else(|| registered_api.map(UpstreamApi::upstream_model))
        else {
            return generation_result(
                protocol,
                mode,
                reasoning_effort,
                None,
                elapsed_millis(&started),
                ProbeResult::unsupported(ProbeFailure::ModelUnavailable),
                None,
            );
        };
        if registered_api.is_some_and(|api| !probe_mode_allowed(api, mode)) {
            return generation_result(
                protocol,
                mode,
                reasoning_effort,
                Some(upstream_model),
                elapsed_millis(&started),
                ProbeResult::unsupported(ProbeFailure::DeliveryUnavailable),
                None,
            );
        }
        let request = probe_text_request(
            protocol,
            upstream_model,
            registered_api
                .map(|api| self.probe_max_output_tokens(api))
                .unwrap_or(PROBE_MAX_OUTPUT_TOKENS),
            mode,
            reasoning_effort,
            allow_unbounded_streaming_output,
        );
        let request = match self.prepare_protocol_request(
            protocol,
            request,
            upstream_model,
            mode == ProbeGenerationMode::Streaming,
        ) {
            Ok(request) => request,
            Err(outcome) => {
                return generation_result(
                    protocol,
                    mode,
                    reasoning_effort,
                    Some(upstream_model),
                    elapsed_millis(&started),
                    outcome,
                    None,
                );
            }
        };

        // Preserve this case independently, including bounded metadata from valid responses.
        let (outcome, evidence) = match mode {
            ProbeGenerationMode::NonStreaming => match self.send_json(request).await {
                Ok(response) => {
                    let evidence =
                        json_generation_evidence(protocol, &response.body, response.content_type);
                    let outcome = if !is_protocol_response(protocol, &response.body) {
                        ProbeResult::inconclusive(
                            Some(response.status),
                            ProbeFailure::UnexpectedResponse,
                        )
                    } else if evidence.terminal == Some(ProbeTerminal::ResponsesFailed) {
                        ProbeResult::rejected(
                            response.status,
                            ProbeFailure::UpstreamTerminalFailure,
                        )
                    } else {
                        ProbeResult::accepted(response.status)
                    };
                    (outcome, Some(evidence))
                }
                Err(outcome) => (outcome, None),
            },
            ProbeGenerationMode::Streaming => self.send_protocol_sse(protocol, request).await,
        };
        generation_result(
            protocol,
            mode,
            reasoning_effort,
            Some(upstream_model),
            elapsed_millis(&started),
            outcome,
            evidence,
        )
    }

    /// Executes one fixed single-text Embeddings Create request.
    async fn probe_embeddings(&self) -> ProbeResult {
        // Resolve the registered Embeddings API and build its fixed request body.
        let Some(upstream_api) = self.target.upstream_api(UpstreamApiKey::new(
            OperationKind::EmbeddingsCreate,
            self.target.canonical_task(),
        )) else {
            return ProbeResult::unsupported(ProbeFailure::OperationUnavailable);
        };
        let body = probe_embedding_request(upstream_api.upstream_model());
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = EmbeddingRequest::new(Bytes::from(body));

        // Bind the request through the fixed adapter and require a recognizable Embeddings response.
        let operation = match self
            .target
            .kind()
            .definition()
            .operation_adapter(OperationKind::EmbeddingsCreate)
        {
            Some(ProviderOperationAdapter::Embeddings(adapter)) => adapter,
            Some(ProviderOperationAdapter::Generation(_))
            | Some(ProviderOperationAdapter::ImagesGenerations(_))
            | None => {
                return ProbeResult::unsupported(ProbeFailure::OperationUnavailable);
            }
        };
        let request = match operation.prepare_routed_request(&request, upstream_api) {
            Ok(request) => request,
            Err(_) => {
                return ProbeResult::inconclusive(None, ProbeFailure::RequestPreparation);
            }
        };
        match self.send_json(request).await {
            Ok(response)
                if is_embedding_response(&response.body, upstream_api.upstream_model()) =>
            {
                ProbeResult::accepted(response.status)
            }
            Ok(response) => {
                ProbeResult::inconclusive(Some(response.status), ProbeFailure::UnexpectedResponse)
            }
            Err(outcome) => outcome,
        }
    }

    /// Consumes one prepared protocol request as bounded SSE until a recognized terminal event.
    async fn send_protocol_sse(
        &self,
        protocol: ApiProtocol,
        request: PreparedUpstreamRequest,
    ) -> (ProbeResult, Option<GenerationProbeEvidence>) {
        // Send the request and validate status and media type before consuming stream bytes.
        let response = match self
            .transport
            .send(self.target, request, self.headers.clone())
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return (
                    ProbeResult::inconclusive(None, ProbeFailure::Transport),
                    None,
                );
            }
        };
        let status = response.status();
        if !status.is_success() {
            return (ProbeResult::from_http_status(status), None);
        }
        let adapter = match self.generation_adapter(protocol) {
            Ok(adapter) => adapter,
            Err(()) => {
                return (
                    ProbeResult::unsupported(ProbeFailure::OperationUnavailable),
                    None,
                );
            }
        };
        let mut evidence = GenerationProbeEvidence {
            content_type: canonical_content_type(response.headers()),
            ..GenerationProbeEvidence::default()
        };
        if !adapter.recognizes_sse_response(response.headers()) {
            return (
                ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidSseMediaType),
                Some(evidence),
            );
        }

        // Decode bounded chunks and stop as soon as the adapter recognizes a terminal event.
        let mut total_bytes = 0usize;
        let mut decoder = SseDecoder::new(self.max_sse_event_bytes);
        let mut stream = response.into_body().into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(_) => {
                    return (
                        ProbeResult::inconclusive(Some(status), ProbeFailure::Transport),
                        Some(evidence),
                    );
                }
            };
            total_bytes = match total_bytes.checked_add(chunk.len()) {
                Some(total) if total <= self.max_response_bytes => total,
                _ => {
                    return (
                        ProbeResult::inconclusive(Some(status), ProbeFailure::ResponseLimit),
                        Some(evidence),
                    );
                }
            };
            let events = match decoder.push(&chunk) {
                Ok(events) => events,
                Err(_) => {
                    return (
                        ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidSse),
                        Some(evidence),
                    );
                }
            };
            if let Some(outcome) = self.classify_sse_events(protocol, status, events, &mut evidence)
            {
                return (outcome, Some(evidence));
            }
        }

        // Finalize an unterminated last event and require an explicit normal terminal.
        let events = match decoder.finish() {
            Ok(events) => events,
            Err(_) => {
                return (
                    ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidSse),
                    Some(evidence),
                );
            }
        };
        let outcome = self
            .classify_sse_events(protocol, status, events, &mut evidence)
            .unwrap_or_else(|| {
                ProbeResult::inconclusive(Some(status), ProbeFailure::MissingTerminal)
            });
        (outcome, Some(evidence))
    }

    /// Classifies framed SSE events and returns a conclusion only for a terminal event.
    fn classify_sse_events(
        &self,
        protocol: ApiProtocol,
        status: StatusCode,
        events: Vec<crate::transport::sse::SseEvent>,
        evidence: &mut GenerationProbeEvidence,
    ) -> Option<ProbeResult> {
        // Delegate lifecycle semantics to the Provider adapter and stop at the first terminal.
        let adapter = self.generation_adapter(protocol).ok()?;
        for event in events {
            let terminal = observe_sse_event(protocol, &event, evidence);
            let event = adapter.classify_sse_event(event).ok()?;
            match event.status() {
                StreamEventStatus::Continue => {}
                StreamEventStatus::Completed => {
                    evidence.terminal = terminal.or(Some(match protocol {
                        ApiProtocol::ChatCompletions => ProbeTerminal::ChatDone,
                        ApiProtocol::Responses => ProbeTerminal::ResponsesCompleted,
                    }));
                    return Some(ProbeResult::accepted(status));
                }
                StreamEventStatus::Failed => {
                    evidence.terminal = terminal;
                    return Some(if terminal == Some(ProbeTerminal::ResponsesIncomplete) {
                        ProbeResult::accepted(status)
                    } else {
                        ProbeResult::rejected(status, ProbeFailure::UpstreamTerminalFailure)
                    });
                }
            }
        }
        None
    }

    /// Serializes and binds one fixed synthetic request through a compile-time Provider path.
    fn prepare_protocol_request(
        &self,
        protocol: ApiProtocol,
        mut body: Value,
        upstream_model: &str,
        streaming: bool,
    ) -> Result<PreparedUpstreamRequest, ProbeResult> {
        // Apply the same fixed trusted instruction policy without accepting arbitrary probe text.
        let default_instructions = self
            .default_instructions
            .ok_or_else(|| ProbeResult::inconclusive(None, ProbeFailure::RequestPreparation))?;
        normalize_probe_generation_request(protocol, &mut body, default_instructions)
            .map_err(|_| ProbeResult::inconclusive(None, ProbeFailure::RequestPreparation))?;
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = ApiRequest::new(protocol, Bytes::from(body));

        // Bind the explicit model only through the Provider's static operation path and body hook.
        self.generation_adapter(protocol)
            .map_err(|_| ProbeResult::unsupported(ProbeFailure::OperationUnavailable))?
            .prepare_probe_request(&request, upstream_model, streaming)
            .map_err(|_| ProbeResult::inconclusive(None, ProbeFailure::RequestPreparation))
    }

    /// Selects the Provider's one typed Generation operation for this probe protocol.
    fn generation_adapter(&self, protocol: ApiProtocol) -> Result<GenerationProviderAdapter, ()> {
        match self
            .target
            .kind()
            .definition()
            .operation_adapter(protocol.operation())
        {
            Some(ProviderOperationAdapter::Generation(adapter)) => Ok(adapter),
            Some(ProviderOperationAdapter::Embeddings(_))
            | Some(ProviderOperationAdapter::ImagesGenerations(_))
            | None => Err(()),
        }
    }

    /// Sends a prepared request and normalizes transport, HTTP, and JSON failures to a conservative outcome.
    async fn send_json(
        &self,
        request: crate::provider::PreparedUpstreamRequest,
    ) -> Result<JsonResponse, ProbeResult> {
        // Send the request and convert transport failures to an inconclusive local stage.
        let response = self
            .transport
            .send(self.target, request, self.headers.clone())
            .await
            .map_err(|_| ProbeResult::inconclusive(None, ProbeFailure::Transport))?;

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

fn generation_result(
    protocol: ApiProtocol,
    mode: ProbeGenerationMode,
    reasoning_effort: ProbeReasoningEffort,
    upstream_model: Option<&str>,
    elapsed_ms: u64,
    outcome: ProbeResult,
    evidence: Option<GenerationProbeEvidence>,
) -> GenerationProbeResult {
    GenerationProbeResult {
        protocol: ProbeProtocol::from_api(protocol),
        mode,
        reasoning_effort,
        upstream_model: upstream_model.map(str::to_owned),
        elapsed_ms,
        outcome,
        evidence,
    }
}

fn probe_mode_allowed(upstream_api: &UpstreamApi, mode: ProbeGenerationMode) -> bool {
    match mode {
        ProbeGenerationMode::Streaming => upstream_api
            .capabilities()
            .generation_capabilities()
            .is_some_and(|capabilities| capabilities.streaming),
        ProbeGenerationMode::NonStreaming => !upstream_api.streaming_policy().requires_streaming(),
    }
}

fn elapsed_millis(started: &Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn json_generation_evidence(
    protocol: ApiProtocol,
    body: &Value,
    content_type: Option<String>,
) -> GenerationProbeEvidence {
    let terminal = match protocol {
        ApiProtocol::ChatCompletions => Some(ProbeTerminal::NonStreaming),
        ApiProtocol::Responses => match body.get("status").and_then(Value::as_str) {
            Some("completed") => Some(ProbeTerminal::ResponsesCompleted),
            Some("incomplete") => Some(ProbeTerminal::ResponsesIncomplete),
            Some("failed") => Some(ProbeTerminal::ResponsesFailed),
            _ => Some(ProbeTerminal::NonStreaming),
        },
    };
    GenerationProbeEvidence {
        content_type,
        terminal,
        usage_present: body.get("usage").is_some_and(Value::is_object),
        usage: body.get("usage").and_then(probe_token_usage),
        output_text_observed: match protocol {
            ApiProtocol::ChatCompletions => body
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        choice
                            .get("message")
                            .and_then(|message| message.get("content"))
                            .is_some_and(|content| !content.is_null())
                    })
                }),
            ApiProtocol::Responses => response_output_has_type(body, "output_text"),
        },
        reasoning_observed: match protocol {
            ApiProtocol::ChatCompletions => body
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        choice.get("message").is_some_and(|message| {
                            ["reasoning_content", "reasoning", "reasoning_details"]
                                .iter()
                                .any(|field| {
                                    message.get(*field).is_some_and(|value| !value.is_null())
                                })
                        })
                    })
                }),
            ApiProtocol::Responses => response_output_has_type(body, "reasoning"),
        },
        event_types: Vec::new(),
    }
}

fn response_output_has_type(body: &Value, expected: &str) -> bool {
    body.get("output")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some(expected)
                    || item
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(|content| {
                            content.iter().any(|part| {
                                part.get("type").and_then(Value::as_str) == Some(expected)
                            })
                        })
            })
        })
}

fn probe_token_usage(usage: &Value) -> Option<ProbeTokenUsage> {
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64);
    let reasoning_tokens = usage
        .pointer("/output_tokens_details/reasoning_tokens")
        .or_else(|| usage.pointer("/completion_tokens_details/reasoning_tokens"))
        .and_then(Value::as_u64);
    let total_tokens = usage.get("total_tokens").and_then(Value::as_u64);
    [input_tokens, output_tokens, reasoning_tokens, total_tokens]
        .iter()
        .any(Option::is_some)
        .then_some(ProbeTokenUsage {
            input_tokens,
            output_tokens,
            reasoning_tokens,
            total_tokens,
        })
}

fn observe_sse_event(
    protocol: ApiProtocol,
    event: &crate::transport::sse::SseEvent,
    evidence: &mut GenerationProbeEvidence,
) -> Option<ProbeTerminal> {
    if protocol == ApiProtocol::ChatCompletions && event.data().trim() == "[DONE]" {
        return Some(ProbeTerminal::ChatDone);
    }

    let document = serde_json::from_str::<Value>(event.data()).ok();
    let event_type = event.event().map(str::to_owned).or_else(|| {
        document
            .as_ref()
            .and_then(|document| document.get("type"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    if let Some(event_type) = event_type.as_deref().filter(|value| safe_event_type(value)) {
        if evidence.event_types.len() < 64
            && !evidence.event_types.iter().any(|value| value == event_type)
        {
            evidence.event_types.push(event_type.to_owned());
        }
        evidence.reasoning_observed |= event_type.contains("reasoning");
        evidence.output_text_observed |= event_type.contains("output_text");
    }

    if let Some(document) = document.as_ref() {
        let usage = document.get("usage").or_else(|| {
            document
                .get("response")
                .and_then(|response| response.get("usage"))
        });
        evidence.usage_present |= usage.is_some_and(Value::is_object);
        if let Some(usage) = usage.and_then(probe_token_usage) {
            evidence.usage = Some(usage);
        }
        if protocol == ApiProtocol::ChatCompletions {
            evidence.output_text_observed |= document
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        choice
                            .get("delta")
                            .and_then(|delta| delta.get("content"))
                            .is_some_and(|content| !content.is_null())
                    })
                });
        }
    }

    match event_type.as_deref() {
        Some("response.completed") => Some(ProbeTerminal::ResponsesCompleted),
        Some("response.incomplete") => Some(ProbeTerminal::ResponsesIncomplete),
        Some("response.failed") => Some(ProbeTerminal::ResponsesFailed),
        _ => None,
    }
}

fn safe_event_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn canonical_content_type(headers: &HeaderMap) -> Option<String> {
    let media_type = headers
        .get(CONTENT_TYPE)?
        .to_str()
        .ok()?
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase();
    (!media_type.is_empty()
        && media_type.len() <= 64
        && media_type
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'+' | b'.')))
    .then_some(media_type)
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
    content_type: Option<String>,
}

/// Reads a successful JSON response within the fixed limit and classifies HTTP failures first.
async fn decode_json_response(
    response: UpstreamResponse,
    max_response_bytes: usize,
) -> Result<JsonResponse, ProbeResult> {
    // Read the response body within the configured limit and classify HTTP failures first.
    let status = response.status();
    let content_type = canonical_content_type(response.headers());
    if !status.is_success() {
        return Err(ProbeResult::from_http_status(status));
    }
    let body = to_bytes(response.into_body(), max_response_bytes)
        .await
        .map_err(|_| ProbeResult::inconclusive(Some(status), ProbeFailure::ResponseLimit))?;

    // Accept only valid JSON so an error page cannot be reported as protocol success.
    let body = serde_json::from_slice(&body)
        .map_err(|_| ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidJson))?;
    Ok(JsonResponse {
        status,
        body,
        content_type,
    })
}
