//! Trusted session for administrative probes against a fixed Upstream Target.
//!
//! The session uses only the endpoint, model, adapter, and startup credential snapshot from the
//! compiled registry. A network or protocol failure produces a conservative outcome for that
//! probe and does not block other probes in the same report.

mod evidence;
mod json_response;

use std::time::Instant;

use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode};
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
    transport::{sse::SseDecoder, upstream::UpstreamTransport},
};

use evidence::{
    GenerationOutputObservation, elapsed_millis, generation_capability_evidence, generation_result,
    json_generation_evidence, json_generation_output, observe_sse_event, observe_sse_tool_event,
    probe_mode_allowed,
};
use json_response::{JsonResponse, canonical_content_type, decode_json_response};

use super::{
    GenerationCaseSelection, GenerationProbeEvidence, GenerationProbeResult, ModelListProbeResult,
    ProbeCapabilityEvidence, ProbeCapabilityVerdict, ProbeError, ProbeFailure,
    ProbeGenerationCapability, ProbeGenerationMode, ProbeOptions, ProbeResult, ProbeStatus,
    ProbeTerminal, TargetProbeReport,
    payload::{
        is_embedding_response, is_protocol_response, probe_embedding_request,
        probe_generation_request,
    },
};

const MAX_REPORTED_MODEL_IDS: usize = 1_024;

/// Resolves one enabled Generation Target for a Provider without accepting endpoint configuration.
pub fn resolve_generation_probe_target(
    registry: &RuntimeRegistry,
    provider: ProviderKind,
    explicit_target: Option<&str>,
) -> Result<String, ProbeError> {
    if let Some(target_id) = explicit_target {
        let target = registry.upstream_target(target_id).ok_or_else(|| {
            ProbeError::UnknownUpstreamTarget {
                upstream_target: target_id.to_owned(),
            }
        })?;
        if !target.enabled()
            || target.kind() != provider
            || target.canonical_task() != CanonicalTaskKind::Generation
        {
            return Err(ProbeError::ProviderTargetMismatch {
                provider: provider.slug().to_owned(),
                upstream_target: target_id.to_owned(),
            });
        }
        return Ok(target_id.to_owned());
    }

    // Group candidate Targets by trusted deployment and credential binding before selecting one.
    let mut candidates = registry
        .upstream_target_ids()
        .filter_map(|target_id| {
            let target = registry.upstream_target(target_id)?;
            (target.enabled()
                && target.kind() == provider
                && target.canonical_task() == CanonicalTaskKind::Generation)
                .then_some((
                    target_id,
                    target.provider_instance_id(),
                    target.credential_pool_id(),
                ))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(ProbeError::ProviderGenerationTargetUnavailable {
            provider: provider.slug().to_owned(),
        });
    }
    candidates.sort_unstable();
    let deployment = (candidates[0].1, candidates[0].2);
    if candidates
        .iter()
        .any(|candidate| (candidate.1, candidate.2) != deployment)
    {
        return Err(ProbeError::AmbiguousProviderTarget {
            provider: provider.slug().to_owned(),
            targets: candidates
                .iter()
                .map(|candidate| candidate.0)
                .collect::<Vec<_>>()
                .join(", "),
        });
    }
    Ok(candidates[0].0.to_owned())
}

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

/// Probes a subscription target by borrowing one guarded OAuth2 access-token lease.
///
/// The manager owns refresh and account binding. This entry point accepts no endpoint or
/// credential override, and it supports exactly the Providers that carry OAuth2 credentials.
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
    if !matches!(target.kind(), ProviderKind::ChatGpt | ProviderKind::Grok) {
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

    // Prepare the fixed subscription authentication headers before entering the common probe session.
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

    // Run each selected unit independently; Generation is deliberately limited to one request.
    let list_models = if selection.list_models {
        Some(
            session
                .probe_list_models(selection.upstream_model.as_deref())
                .await,
        )
    } else {
        None
    };
    let generation = if let Some(generation) = selection.generation {
        Some(
            session
                .probe_generation_case(
                    &GenerationCaseSelection {
                        protocol: generation.protocol.as_api(),
                        mode: generation.mode,
                        case: generation.case,
                        custom_prompt: generation.custom_prompt,
                        custom_schema: generation.custom_schema,
                        custom_schema_name: generation.custom_schema_name,
                    },
                    selection.upstream_model.as_deref(),
                    selection.allow_unbounded_streaming_output,
                )
                .await,
        )
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

    /// Executes one fixed synthetic Generation request without registered model-specific rewrites.
    async fn probe_generation_case(
        &self,
        case: &GenerationCaseSelection,
        upstream_model_override: Option<&str>,
        allow_unbounded_streaming_output: bool,
    ) -> GenerationProbeResult {
        let started = Instant::now();
        if self.target.canonical_task() != CanonicalTaskKind::Generation {
            return generation_result(
                case,
                upstream_model_override,
                elapsed_millis(&started),
                ProbeResult::unsupported(ProbeFailure::OperationUnavailable),
                None,
                None,
            );
        }
        // Use an explicit candidate model, or the registered model for this exact protocol.
        let registered_api = self.target.upstream_api(UpstreamApiKey::new(
            case.protocol.operation(),
            self.target.canonical_task(),
        ));
        let Some(upstream_model) =
            upstream_model_override.or_else(|| registered_api.map(UpstreamApi::upstream_model))
        else {
            return generation_result(
                case,
                None,
                elapsed_millis(&started),
                ProbeResult::unsupported(ProbeFailure::ModelUnavailable),
                None,
                None,
            );
        };
        if upstream_model_override.is_none()
            && registered_api.is_some_and(|api| !probe_mode_allowed(api, case.mode))
        {
            return generation_result(
                case,
                Some(upstream_model),
                elapsed_millis(&started),
                ProbeResult::unsupported(ProbeFailure::DeliveryUnavailable),
                None,
                None,
            );
        }
        let request = probe_generation_request(
            upstream_model,
            if upstream_model_override.is_some() {
                case.capability().max_output_tokens()
            } else {
                registered_api
                    .map(|api| self.probe_max_output_tokens(api, case.capability()))
                    .unwrap_or_else(|| case.capability().max_output_tokens())
            },
            allow_unbounded_streaming_output,
            case,
        );
        let request = match self.prepare_protocol_request(
            case.protocol,
            request,
            upstream_model,
            case.mode == ProbeGenerationMode::Streaming,
        ) {
            Ok(request) => request,
            Err(outcome) => {
                return generation_result(
                    case,
                    Some(upstream_model),
                    elapsed_millis(&started),
                    outcome,
                    None,
                    None,
                );
            }
        };

        // Preserve this case independently, including bounded metadata from valid responses.
        let (outcome, evidence, output) = match case.mode {
            ProbeGenerationMode::NonStreaming => match self.send_json(request).await {
                Ok(response) => {
                    let output = json_generation_output(case.protocol, &response.body);
                    let evidence = json_generation_evidence(
                        case.protocol,
                        &response.body,
                        response.content_type,
                    );
                    let outcome = if !is_protocol_response(case.protocol, &response.body) {
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
                    (outcome, Some(evidence), output)
                }
                Err(outcome) => (outcome, None, GenerationOutputObservation::default()),
            },
            ProbeGenerationMode::Streaming => self.send_protocol_sse(case.protocol, request).await,
        };
        let capability_evidence = (outcome.state == ProbeStatus::Accepted).then(|| {
            let evidence = generation_capability_evidence(
                case.capability(),
                &output,
                evidence.as_ref().and_then(|evidence| evidence.terminal),
            );
            // An admin-authored schema removes the fixed oracle; acceptance stays observable,
            // but no compliance verdict can be rendered against an arbitrary schema.
            if case.custom_schema.is_some() {
                ProbeCapabilityEvidence {
                    verdict: ProbeCapabilityVerdict::Inconclusive,
                    fixed_schema_match: None,
                    ..evidence
                }
            } else {
                evidence
            }
        });
        generation_result(
            case,
            Some(upstream_model),
            elapsed_millis(&started),
            outcome,
            evidence,
            capability_evidence,
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
    ) -> (
        ProbeResult,
        Option<GenerationProbeEvidence>,
        GenerationOutputObservation,
    ) {
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
                    GenerationOutputObservation::default(),
                );
            }
        };
        let status = response.status();
        if !status.is_success() {
            return (
                ProbeResult::from_http_status(status),
                None,
                GenerationOutputObservation::default(),
            );
        }
        let adapter = match self.generation_adapter(protocol) {
            Ok(adapter) => adapter,
            Err(()) => {
                return (
                    ProbeResult::unsupported(ProbeFailure::OperationUnavailable),
                    None,
                    GenerationOutputObservation::default(),
                );
            }
        };
        let mut evidence = GenerationProbeEvidence {
            content_type: canonical_content_type(response.headers()),
            ..GenerationProbeEvidence::default()
        };
        let mut output = GenerationOutputObservation::new();
        if !adapter.recognizes_sse_response(response.headers()) {
            return (
                ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidSseMediaType),
                Some(evidence),
                GenerationOutputObservation::default(),
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
                        GenerationOutputObservation::default(),
                    );
                }
            };
            total_bytes = match total_bytes.checked_add(chunk.len()) {
                Some(total) if total <= self.max_response_bytes => total,
                _ => {
                    return (
                        ProbeResult::inconclusive(Some(status), ProbeFailure::ResponseLimit),
                        Some(evidence),
                        GenerationOutputObservation::default(),
                    );
                }
            };
            let events = match decoder.push(&chunk) {
                Ok(events) => events,
                Err(_) => {
                    return (
                        ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidSse),
                        Some(evidence),
                        GenerationOutputObservation::default(),
                    );
                }
            };
            if let Some(outcome) =
                self.classify_sse_events(protocol, status, events, &mut evidence, &mut output)
            {
                return (outcome, Some(evidence), output);
            }
        }

        // Finalize an unterminated last event and require an explicit normal terminal.
        let events = match decoder.finish() {
            Ok(events) => events,
            Err(_) => {
                return (
                    ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidSse),
                    Some(evidence),
                    GenerationOutputObservation::default(),
                );
            }
        };
        let outcome = self
            .classify_sse_events(protocol, status, events, &mut evidence, &mut output)
            .unwrap_or_else(|| {
                ProbeResult::inconclusive(Some(status), ProbeFailure::MissingTerminal)
            });
        (outcome, Some(evidence), output)
    }

    /// Classifies framed SSE events and returns a conclusion only for a terminal event.
    fn classify_sse_events(
        &self,
        protocol: ApiProtocol,
        status: StatusCode,
        events: Vec<crate::transport::sse::SseEvent>,
        evidence: &mut GenerationProbeEvidence,
        output: &mut GenerationOutputObservation,
    ) -> Option<ProbeResult> {
        // Delegate lifecycle semantics to the Provider adapter and stop at the first terminal.
        let adapter = self.generation_adapter(protocol).ok()?;
        for event in events {
            observe_sse_tool_event(protocol, &event, output);
            let terminal = observe_sse_event(protocol, &event, evidence, &mut output.text);
            let event = adapter.classify_sse_event(event).ok()?;
            match event.status() {
                StreamEventStatus::Continue => {}
                StreamEventStatus::Completed => {
                    output.finish_stream_tool_calls();
                    evidence.terminal = terminal.or(Some(match protocol {
                        ApiProtocol::ChatCompletions => ProbeTerminal::ChatDone,
                        ApiProtocol::Responses => ProbeTerminal::ResponsesCompleted,
                    }));
                    return Some(ProbeResult::accepted(status));
                }
                StreamEventStatus::Failed => {
                    output.finish_stream_tool_calls();
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
    fn probe_max_output_tokens(
        &self,
        upstream_api: &UpstreamApi,
        capability: ProbeGenerationCapability,
    ) -> u32 {
        let safety_limit = capability.max_output_tokens();
        upstream_api
            .model()
            .context_length()
            .output_tokens()
            .unwrap_or(safety_limit)
            .min(safety_limit)
    }
}
