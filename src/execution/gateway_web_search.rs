//! Gateway-executed bounded read-only web search loop over canonical Generation IR.
//!
//! The executor owns only the internal turn/tool budget for one fixed candidate: it buffers
//! intermediate Provider turns, executes the trusted read-only search seam, aggregates successful
//! turn usage, and returns the final canonical response. Routing, Provider selection, transport
//! wiring, credentials, and downstream commit remain with their existing owners.

use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::ir::generation::{
    CallId, FinishReason, GenerationRequest, GenerationResponse, ResponseStatus, ServerToolConfig,
    ToolCall, ToolExecutor, ToolName, ToolOrigin, ToolOutput, ToolPlanId, ToolResult,
    ToolResultStatus, ToolVisibility, Usage,
};

const MAX_SEARCH_OUTPUT_BYTES: usize = 32 * 1024;

/// One physical Provider turn attempt through an injected driver seam.
pub struct GatewayTurn {
    /// Exact Provider origin that produced this turn.
    pub origin: crate::ir::generation::ProviderOrigin,
    /// Validated canonical response for this internal turn.
    pub response: GenerationResponse,
    /// Usage reported by this turn, when present.
    pub usage: Option<Usage>,
}

/// Failure of one physical Provider turn attempt.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TurnError {
    /// The turn driver exceeded its per-attempt deadline or was cancelled.
    #[error("gateway turn attempt exceeded its deadline")]
    Deadline,
    /// The turn driver failed without a Provider response.
    #[error("gateway turn attempt failed")]
    Attempt,
}

/// One bounded read-only search record produced by the trusted search seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRecord {
    title: String,
    url: String,
    snippet: String,
}

impl SearchRecord {
    /// Creates one search record with bounded, non-empty fields.
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        snippet: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, SearchError> {
        let title = bounded_field(title, max_bytes)?;
        let url = bounded_field(url, max_bytes)?;
        let snippet = bounded_field(snippet, max_bytes)?;
        Ok(Self {
            title,
            url,
            snippet,
        })
    }

    /// Returns the record title.
    pub const fn title(&self) -> &String {
        &self.title
    }

    /// Returns the record URL.
    pub const fn url(&self) -> &String {
        &self.url
    }

    /// Returns the record snippet.
    pub const fn snippet(&self) -> &String {
        &self.snippet
    }
}

/// Failure of one read-only search execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SearchError {
    /// The search call exceeded its deadline or was cancelled.
    #[error("gateway search call exceeded its deadline")]
    Deadline,
    /// The trusted search seam failed without records.
    #[error("gateway search call failed")]
    Failed,
}

fn bounded_field(value: impl Into<String>, max_bytes: usize) -> Result<String, SearchError> {
    let value = value.into();
    if value.is_empty() || value.len() > max_bytes {
        return Err(SearchError::Failed);
    }
    Ok(value)
}

/// One physical Provider turn attempt injected by the trusted composition root.
pub trait GatewayTurnDriver {
    /// Executes one physical Provider attempt against one fixed origin.
    fn attempt<'a>(
        &'a self,
        request: &'a GenerationRequest,
        deadline: Instant,
        cancel: &'a CancelToken,
    ) -> Pin<Box<dyn Future<Output = Result<GatewayTurn, TurnError>> + 'a>>;
}

/// One trusted read-only web-search execution injected by the trusted composition root.
pub trait ReadOnlyWebSearch {
    /// Executes one bounded search call for the given Gateway tool call.
    fn search<'a>(
        &'a self,
        call: &'a ToolCall,
        remaining_results: usize,
        deadline: Instant,
        cancel: &'a CancelToken,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SearchRecord>, SearchError>> + 'a>>;
}

/// Cooperative cancellation token shared across the loop and both injected seams.
#[derive(Clone, Default)]
pub struct CancelToken {
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CancelToken {
    /// Creates a live token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the token cancelled; running checks observe it at the next await point.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Returns whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Trusted non-zero budgets for one Gateway web-search operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatewayWebSearchLimits {
    max_turns: usize,
    max_tool_calls: usize,
    max_results: usize,
    max_attempts: usize,
    deadline: Duration,
    max_record_bytes: usize,
}

impl GatewayWebSearchLimits {
    /// Creates non-zero budgets from trusted constructor policy.
    pub fn new(
        max_turns: usize,
        max_tool_calls: usize,
        max_results: usize,
        max_attempts: usize,
        deadline: Duration,
    ) -> Result<Self, GatewayWebSearchError> {
        if max_turns == 0
            || max_tool_calls == 0
            || max_results == 0
            || max_attempts == 0
            || deadline.is_zero()
        {
            return Err(GatewayWebSearchError::InvalidLimits);
        }
        Ok(Self {
            max_turns,
            max_tool_calls,
            max_results,
            max_attempts,
            deadline,
            max_record_bytes: 4 * 1024,
        })
    }
}

/// The trusted fixed candidate bound to one logical Gateway search operation.
#[derive(Clone, Debug)]
pub struct FixedCandidate {
    /// Immutable Provider origin compiled from the prepared Route plan.
    origin: crate::ir::generation::ProviderOrigin,
    /// Trusted Gateway web-search tool name injected by the plan.
    tool_name: ToolName,
    /// Name of the web-search tool as the Provider must report calls.
    call_name: ToolName,
    /// Exact trusted plan that injected the reserved Gateway tool.
    tool_plan: ToolPlanId,
}

impl FixedCandidate {
    /// Binds one operation to one fixed origin and tool identity.
    pub fn new(
        origin: crate::ir::generation::ProviderOrigin,
        tool_name: ToolName,
        call_name: ToolName,
        tool_plan: ToolPlanId,
    ) -> Self {
        Self {
            origin,
            tool_name,
            call_name,
            tool_plan,
        }
    }
}

/// Final buffered outcome of one successful Gateway web-search operation.
#[derive(Debug)]
pub struct GatewayWebSearchResult {
    /// Final validated response to render downstream.
    pub final_response: GenerationResponse,
    /// Sum of every successful turn usage; missing fields stay missing.
    pub aggregate_usage: Option<Usage>,
    /// Successful Provider turns consumed, including the tool-call turn.
    pub turns: usize,
    /// Gateway web-search executions performed.
    pub tool_calls: usize,
    /// Physical Provider attempts started, including failed ones.
    pub attempts: usize,
}

/// Typed failure of one Gateway web-search operation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GatewayWebSearchError {
    /// Constructor budgets were zero.
    #[error("gateway web search limits must be non-zero")]
    InvalidLimits,
    /// The request does not declare the trusted Gateway web-search tool.
    #[error("gateway web search request lacks the trusted tool")]
    InvalidRequest,
    /// The declared tool is not a Gateway-executed web search.
    #[error("gateway web search tool identity is invalid")]
    InvalidToolIdentity,
    /// The Provider turn reported a different origin than the fixed candidate.
    #[error("gateway web search turn arrived from a different origin")]
    OriginMismatch,
    /// A canonical turn had an invalid status, candidate count, or finish/tool lifecycle.
    #[error("gateway web search turn lifecycle is invalid")]
    InvalidTurnLifecycle,
    /// A turn budget was exhausted.
    #[error("gateway web search exceeded the turn budget")]
    TurnLimitExceeded,
    /// A tool-call budget was exhausted.
    #[error("gateway web search exceeded the tool-call budget")]
    ToolCallLimitExceeded,
    /// A result budget was exhausted.
    #[error("gateway web search exceeded the result budget")]
    ResultLimitExceeded,
    /// A physical attempt budget was exhausted.
    #[error("gateway web search exceeded the attempt budget")]
    AttemptLimitExceeded,
    /// The absolute deadline expired.
    #[error("gateway web search exceeded its deadline")]
    Deadline,
    /// Cancellation was requested before completion.
    #[error("gateway web search was cancelled")]
    Cancelled,
    /// The search seam failed after its bounded behavior.
    #[error("gateway web search failed")]
    SearchFailed,
    /// Aggregate usage arithmetic overflowed.
    #[error("gateway web search usage aggregate overflowed")]
    UsageOverflow,
}

/// Bounded Gateway-executed web-search loop over one fixed candidate.
pub struct GatewayWebSearchExecutor<'a, D, S> {
    driver: &'a D,
    search: &'a S,
    candidate: FixedCandidate,
    limits: GatewayWebSearchLimits,
}

impl<'a, D, S> GatewayWebSearchExecutor<'a, D, S>
where
    D: GatewayTurnDriver,
    S: ReadOnlyWebSearch,
{
    /// Binds the injected seams, fixed candidate, and trusted budgets once.
    pub fn new(
        driver: &'a D,
        search: &'a S,
        candidate: FixedCandidate,
        limits: GatewayWebSearchLimits,
    ) -> Result<Self, GatewayWebSearchError> {
        if limits.deadline.is_zero() {
            return Err(GatewayWebSearchError::InvalidLimits);
        }
        Ok(Self {
            driver,
            search,
            candidate,
            limits,
        })
    }

    /// Runs the complete buffered loop to a terminal canonical response.
    pub async fn execute(
        &self,
        request: GenerationRequest,
        cancel: &CancelToken,
    ) -> Result<GatewayWebSearchResult, GatewayWebSearchError> {
        self.validate_request(&request)?;
        let deadline = Instant::now() + self.limits.deadline;
        let mut aggregate: Option<Usage> = None;
        let mut turns = 0usize;
        let mut tool_calls = 0usize;
        let mut attempts = 0usize;
        let mut results = 0usize;
        let mut current = request;

        loop {
            if cancel.is_cancelled() {
                return Err(GatewayWebSearchError::Cancelled);
            }
            if turns >= self.limits.max_turns {
                return Err(GatewayWebSearchError::TurnLimitExceeded);
            }
            if attempts >= self.limits.max_attempts {
                return Err(GatewayWebSearchError::AttemptLimitExceeded);
            }
            if Instant::now() >= deadline {
                return Err(GatewayWebSearchError::Deadline);
            }
            attempts += 1;
            let turn = match self.driver.attempt(&current, deadline, cancel).await {
                Ok(turn) => turn,
                Err(TurnError::Deadline) => return Err(GatewayWebSearchError::Deadline),
                Err(TurnError::Attempt) => continue,
            };
            if cancel.is_cancelled() {
                return Err(GatewayWebSearchError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(GatewayWebSearchError::Deadline);
            }
            if turn.origin != self.candidate.origin {
                return Err(GatewayWebSearchError::OriginMismatch);
            }
            if turn.response.status() != ResponseStatus::Completed {
                return Err(GatewayWebSearchError::InvalidTurnLifecycle);
            }
            let [candidate] = turn.response.candidates() else {
                return Err(GatewayWebSearchError::InvalidTurnLifecycle);
            };
            let has_tool_call = candidate
                .output()
                .iter()
                .any(|item| matches!(item, crate::ir::generation::OutputItem::ToolCall(_)));
            let needs_tool = match candidate.finish() {
                Some(FinishReason::ToolCalls) if has_tool_call => true,
                Some(FinishReason::ToolCalls) | None => {
                    return Err(GatewayWebSearchError::InvalidTurnLifecycle);
                }
                Some(_) if has_tool_call => {
                    return Err(GatewayWebSearchError::InvalidTurnLifecycle);
                }
                Some(_) => false,
            };
            turns += 1;
            aggregate = merge_usage(aggregate, turn.usage)?;

            // A completed turn without tool calls is the final buffered response.
            if !needs_tool {
                return Ok(GatewayWebSearchResult {
                    final_response: turn.response,
                    aggregate_usage: aggregate,
                    turns,
                    tool_calls,
                    attempts,
                });
            }
            if tool_calls >= self.limits.max_tool_calls {
                return Err(GatewayWebSearchError::ToolCallLimitExceeded);
            }
            let call = self.extract_call(&turn.response)?;
            tool_calls += 1;

            // Reserve the continuation turn before starting external search work.
            if turns >= self.limits.max_turns {
                return Err(GatewayWebSearchError::TurnLimitExceeded);
            }
            if attempts >= self.limits.max_attempts {
                return Err(GatewayWebSearchError::AttemptLimitExceeded);
            }
            if results >= self.limits.max_results {
                return Err(GatewayWebSearchError::ResultLimitExceeded);
            }
            let remaining = self.limits.max_results - results;
            if Instant::now() >= deadline {
                return Err(GatewayWebSearchError::Deadline);
            }
            let records = self
                .search
                .search(&call, remaining, deadline, cancel)
                .await
                .map_err(|error| match error {
                    SearchError::Deadline => GatewayWebSearchError::Deadline,
                    SearchError::Failed => GatewayWebSearchError::SearchFailed,
                })?;
            if cancel.is_cancelled() {
                return Err(GatewayWebSearchError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(GatewayWebSearchError::Deadline);
            }
            if records.len() > remaining {
                return Err(GatewayWebSearchError::ResultLimitExceeded);
            }
            results += records.len();
            let output = self.search_output(&records)?;
            let call_id = call.call_id().clone();
            let text = crate::ir::generation::TextValue::new(output, MAX_SEARCH_OUTPUT_BYTES)
                .map_err(|_| GatewayWebSearchError::ResultLimitExceeded)?;
            let result = ToolResult::new(
                item_id_for(&call_id)?,
                call_id.clone(),
                ToolResultStatus::Success,
                vec![ToolOutput::Text(text)],
                None,
            );
            current = current
                .with_appended_input([
                    crate::ir::generation::InputItem::PriorToolCall(call),
                    crate::ir::generation::InputItem::ToolResult(result),
                ])
                .map_err(|_| GatewayWebSearchError::InvalidRequest)?;
        }
    }

    fn validate_request(&self, request: &GenerationRequest) -> Result<(), GatewayWebSearchError> {
        let tool = request
            .tools()
            .iter()
            .find(|tool| tool.name() == &self.candidate.tool_name)
            .ok_or(GatewayWebSearchError::InvalidRequest)?;
        if !matches!(
            tool.kind(),
            crate::ir::generation::ToolKind::Server(ServerToolConfig::WebSearch)
        ) || tool.executor() != &ToolExecutor::Gateway
            || tool.visibility() != ToolVisibility::Internal
            || tool.origin() != &ToolOrigin::GatewayPolicy(self.candidate.tool_plan.clone())
        {
            return Err(GatewayWebSearchError::InvalidToolIdentity);
        }
        Ok(())
    }

    fn extract_call(
        &self,
        response: &GenerationResponse,
    ) -> Result<ToolCall, GatewayWebSearchError> {
        let [candidate] = response.candidates() else {
            return Err(GatewayWebSearchError::InvalidToolIdentity);
        };
        let mut calls = candidate.output().iter().filter_map(|item| {
            let crate::ir::generation::OutputItem::ToolCall(call) = item else {
                return None;
            };
            Some(call)
        });
        let call = calls
            .next()
            .ok_or(GatewayWebSearchError::InvalidToolIdentity)?;
        if calls.next().is_some()
            || call.tool() != &self.candidate.call_name
            || !matches!(
                call.input(),
                crate::ir::generation::ToolInput::Server(input)
                    if input.kind() == crate::ir::generation::ServerToolKind::WebSearch
            )
        {
            return Err(GatewayWebSearchError::InvalidToolIdentity);
        }
        Ok(call.clone())
    }

    fn search_output(&self, records: &[SearchRecord]) -> Result<String, GatewayWebSearchError> {
        let mut output = String::new();
        for record in records {
            for value in [record.title(), record.url(), record.snippet()] {
                let next = output
                    .len()
                    .checked_add(value.len())
                    .and_then(|bytes| bytes.checked_add(1))
                    .ok_or(GatewayWebSearchError::ResultLimitExceeded)?;
                if next > MAX_SEARCH_OUTPUT_BYTES {
                    return Err(GatewayWebSearchError::ResultLimitExceeded);
                }
                output.push_str(value);
                output.push('\n');
            }
        }
        Ok(output)
    }
}

fn merge_usage(
    current: Option<Usage>,
    incoming: Option<Usage>,
) -> Result<Option<Usage>, GatewayWebSearchError> {
    let Some(incoming) = incoming else {
        return Ok(current);
    };
    let Some(current) = current else {
        return Ok(Some(incoming));
    };
    let merged = Usage::new(
        checked_sum(current.input_tokens(), incoming.input_tokens())?,
        checked_sum(current.output_tokens(), incoming.output_tokens())?,
        checked_sum(current.total_tokens(), incoming.total_tokens())?,
        checked_sum(current.reasoning_tokens(), incoming.reasoning_tokens())?,
        checked_sum(
            current.cached_input_tokens(),
            incoming.cached_input_tokens(),
        )?,
    );
    Ok(Some(merged))
}

fn checked_sum(
    left: Option<u64>,
    right: Option<u64>,
) -> Result<Option<u64>, GatewayWebSearchError> {
    match (left, right) {
        (Some(left), Some(right)) => left
            .checked_add(right)
            .map(Some)
            .ok_or(GatewayWebSearchError::UsageOverflow),
        _ => Ok(None),
    }
}

fn item_id_for(call: &CallId) -> Result<crate::ir::generation::ItemId, GatewayWebSearchError> {
    let value = format!("fc_{}", call.as_str());
    crate::ir::generation::ItemId::new(value, 256)
        .map_err(|_| GatewayWebSearchError::InvalidToolIdentity)
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use serde_json::json;

    use super::*;
    use crate::ir::generation::{
        Candidate, CandidateId, InputItem, ItemId, JsonObject, OutputItem, ParallelToolCalls,
        ProviderNamespace, ProviderOrigin, ResponseId, ResponseStatus, ServerToolInput,
        ServerToolKind, ToolChoice, ToolDefinition, ToolInput, ToolKind, ToolOrigin, ToolPlanId,
    };

    struct FakeDriver {
        turns: Mutex<VecDeque<Result<GatewayTurn, TurnError>>>,
        requests: Mutex<Vec<GenerationRequest>>,
    }

    impl FakeDriver {
        fn new(turns: impl IntoIterator<Item = Result<GatewayTurn, TurnError>>) -> Self {
            Self {
                turns: Mutex::new(turns.into_iter().collect()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl GatewayTurnDriver for FakeDriver {
        fn attempt<'a>(
            &'a self,
            request: &'a GenerationRequest,
            _deadline: Instant,
            _cancel: &'a CancelToken,
        ) -> Pin<Box<dyn Future<Output = Result<GatewayTurn, TurnError>> + 'a>> {
            self.requests.lock().unwrap().push(request.clone());
            let turn = self.turns.lock().unwrap().pop_front().unwrap();
            Box::pin(async move { turn })
        }
    }

    struct CancelingDriver {
        turn: Mutex<Option<GatewayTurn>>,
    }

    impl GatewayTurnDriver for CancelingDriver {
        fn attempt<'a>(
            &'a self,
            _request: &'a GenerationRequest,
            _deadline: Instant,
            cancel: &'a CancelToken,
        ) -> Pin<Box<dyn Future<Output = Result<GatewayTurn, TurnError>> + 'a>> {
            let turn = self.turn.lock().unwrap().take().unwrap();
            Box::pin(async move {
                cancel.cancel();
                Ok(turn)
            })
        }
    }

    struct DelayedDriver {
        delay: Duration,
        turn: Mutex<Option<GatewayTurn>>,
    }

    impl GatewayTurnDriver for DelayedDriver {
        fn attempt<'a>(
            &'a self,
            _request: &'a GenerationRequest,
            _deadline: Instant,
            _cancel: &'a CancelToken,
        ) -> Pin<Box<dyn Future<Output = Result<GatewayTurn, TurnError>> + 'a>> {
            let delay = self.delay;
            let turn = self.turn.lock().unwrap().take().unwrap();
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                Ok(turn)
            })
        }
    }

    struct FakeSearch {
        results: Mutex<VecDeque<Result<Vec<SearchRecord>, SearchError>>>,
    }

    impl FakeSearch {
        fn new(results: impl IntoIterator<Item = Result<Vec<SearchRecord>, SearchError>>) -> Self {
            Self {
                results: Mutex::new(results.into_iter().collect()),
            }
        }
    }

    impl ReadOnlyWebSearch for FakeSearch {
        fn search<'a>(
            &'a self,
            _call: &'a ToolCall,
            _remaining_results: usize,
            _deadline: Instant,
            _cancel: &'a CancelToken,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<SearchRecord>, SearchError>> + 'a>> {
            let result = self.results.lock().unwrap().pop_front().unwrap();
            Box::pin(async move { result })
        }
    }

    struct DelayedSearch {
        delay: Duration,
        results: Mutex<Option<Vec<SearchRecord>>>,
    }

    impl ReadOnlyWebSearch for DelayedSearch {
        fn search<'a>(
            &'a self,
            _call: &'a ToolCall,
            _remaining_results: usize,
            _deadline: Instant,
            _cancel: &'a CancelToken,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<SearchRecord>, SearchError>> + 'a>> {
            let delay = self.delay;
            let results = self.results.lock().unwrap().take().unwrap();
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                Ok(results)
            })
        }
    }

    fn origin(value: &str) -> ProviderOrigin {
        ProviderOrigin::new(ProviderNamespace::new("test", 64).unwrap(), value, 128).unwrap()
    }

    fn request() -> GenerationRequest {
        request_with_origin(ToolOrigin::GatewayPolicy(
            ToolPlanId::new("plan-r6", 64).unwrap(),
        ))
    }

    fn request_with_origin(tool_origin: ToolOrigin) -> GenerationRequest {
        GenerationRequest::new(Vec::new())
            .unwrap()
            .with_tools(
                vec![ToolDefinition::new(
                    ToolName::new("gateway_web_search", 64).unwrap(),
                    tool_origin,
                    ToolExecutor::Gateway,
                    ToolVisibility::Internal,
                    ToolKind::Server(ServerToolConfig::WebSearch),
                )],
                ToolChoice::Auto,
                ParallelToolCalls::Inactive,
            )
            .unwrap()
    }

    fn fixed_candidate(origin: ProviderOrigin) -> FixedCandidate {
        FixedCandidate::new(
            origin,
            ToolName::new("gateway_web_search", 64).unwrap(),
            ToolName::new("web_search", 64).unwrap(),
            ToolPlanId::new("plan-r6", 64).unwrap(),
        )
    }

    fn call(suffix: &str, input: ToolInput) -> ToolCall {
        ToolCall::new(
            ItemId::new(format!("call-item-{suffix}"), 64).unwrap(),
            CallId::new(format!("call-{suffix}"), 64).unwrap(),
            ToolName::new("web_search", 64).unwrap(),
            input,
            None,
        )
    }

    fn web_search_input() -> ToolInput {
        ToolInput::Server(ServerToolInput::new(
            ServerToolKind::WebSearch,
            JsonObject::new(json!({"query": "rust gateway"}), 256).unwrap(),
        ))
    }

    fn tool_turn(origin: ProviderOrigin, suffix: &str, usage: Option<Usage>) -> GatewayTurn {
        let call = call(suffix, web_search_input());
        let candidate = Candidate::new(
            CandidateId::new(format!("candidate-{suffix}"), 64).unwrap(),
            vec![OutputItem::ToolCall(call)],
            Some(FinishReason::ToolCalls),
        )
        .unwrap();
        GatewayTurn {
            origin,
            response: GenerationResponse::new(
                ResponseId::new(format!("response-{suffix}"), 64).unwrap(),
                vec![candidate],
                ResponseStatus::Completed,
                usage,
                Vec::new(),
            )
            .unwrap(),
            usage,
        }
    }

    fn final_turn(origin: ProviderOrigin, usage: Option<Usage>) -> GatewayTurn {
        let candidate = Candidate::new(
            CandidateId::new("candidate-final", 64).unwrap(),
            Vec::new(),
            Some(FinishReason::Stop),
        )
        .unwrap();
        GatewayTurn {
            origin,
            response: GenerationResponse::new(
                ResponseId::new("response-final", 64).unwrap(),
                vec![candidate],
                ResponseStatus::Completed,
                usage,
                Vec::new(),
            )
            .unwrap(),
            usage,
        }
    }

    fn record() -> SearchRecord {
        SearchRecord::new(
            "Rust",
            "https://example.test/rust",
            "A systems language",
            256,
        )
        .unwrap()
    }

    fn limits(
        max_turns: usize,
        max_tools: usize,
        max_results: usize,
        max_attempts: usize,
    ) -> GatewayWebSearchLimits {
        GatewayWebSearchLimits::new(
            max_turns,
            max_tools,
            max_results,
            max_attempts,
            Duration::from_secs(1),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn successful_loop_preserves_request_semantics_and_correlates_history() {
        let provider = origin("provider-a");
        let first_usage = Usage::new(Some(2), None, None, None, None);
        let final_usage = Usage::new(Some(3), Some(4), Some(7), None, None);
        let driver = FakeDriver::new([
            Ok(tool_turn(provider.clone(), "one", Some(first_usage))),
            Ok(final_turn(provider.clone(), Some(final_usage))),
        ]);
        let search = FakeSearch::new([Ok(vec![record()])]);
        let original = request();
        let executor = GatewayWebSearchExecutor::new(
            &driver,
            &search,
            fixed_candidate(provider),
            limits(2, 1, 1, 2),
        )
        .unwrap();

        let result = executor
            .execute(original.clone(), &CancelToken::new())
            .await
            .unwrap();

        assert_eq!(
            (result.turns, result.tool_calls, result.attempts),
            (2, 1, 2)
        );
        assert_eq!(result.final_response.status(), ResponseStatus::Completed);
        let usage = result.aggregate_usage.unwrap();
        assert_eq!(usage.input_tokens(), Some(5));
        assert_eq!(usage.output_tokens(), None);
        let requests = driver.requests.lock().unwrap();
        assert_eq!(requests[1].tools(), original.tools());
        assert_eq!(requests[1].tool_choice(), original.tool_choice());
        let [
            InputItem::PriorToolCall(call),
            InputItem::ToolResult(tool_result),
        ] = requests[1].input()
        else {
            panic!("continuation must append one correlated call/result pair");
        };
        assert_eq!(call.call_id(), tool_result.call_id());
    }

    #[tokio::test]
    async fn failed_attempt_retries_only_the_fixed_candidate() {
        let provider = origin("provider-a");
        let driver = FakeDriver::new([
            Err(TurnError::Attempt),
            Ok(final_turn(provider.clone(), None)),
        ]);
        let search = FakeSearch::new([]);
        let executor = GatewayWebSearchExecutor::new(
            &driver,
            &search,
            fixed_candidate(provider),
            limits(1, 1, 1, 2),
        )
        .unwrap();

        let result = executor
            .execute(request(), &CancelToken::new())
            .await
            .unwrap();

        assert_eq!((result.turns, result.attempts), (1, 2));
        assert_eq!(driver.requests.lock().unwrap().len(), 2);
    }

    #[test]
    fn provider_tool_turn_requires_one_typed_web_search_call() {
        let provider = origin("provider-a");
        let driver = FakeDriver::new([]);
        let search = FakeSearch::new([]);
        let executor = GatewayWebSearchExecutor::new(
            &driver,
            &search,
            fixed_candidate(provider),
            limits(2, 1, 1, 2),
        )
        .unwrap();

        let function_call = call(
            "function",
            ToolInput::Function(JsonObject::new(json!({"query": "rust"}), 256).unwrap()),
        );
        let candidate = Candidate::new(
            CandidateId::new("candidate-function", 64).unwrap(),
            vec![OutputItem::ToolCall(function_call)],
            Some(FinishReason::ToolCalls),
        )
        .unwrap();
        let response = GenerationResponse::new(
            ResponseId::new("response-function", 64).unwrap(),
            vec![candidate],
            ResponseStatus::Completed,
            None,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            executor.extract_call(&response).unwrap_err(),
            GatewayWebSearchError::InvalidToolIdentity
        );

        let candidate = Candidate::new(
            CandidateId::new("candidate-parallel", 64).unwrap(),
            vec![
                OutputItem::ToolCall(call("parallel-a", web_search_input())),
                OutputItem::ToolCall(call("parallel-b", web_search_input())),
            ],
            Some(FinishReason::ToolCalls),
        )
        .unwrap();
        let response = GenerationResponse::new(
            ResponseId::new("response-parallel", 64).unwrap(),
            vec![candidate],
            ResponseStatus::Completed,
            None,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            executor.extract_call(&response).unwrap_err(),
            GatewayWebSearchError::InvalidToolIdentity
        );
    }

    #[tokio::test]
    async fn independent_attempt_turn_tool_and_result_budgets_fail_closed() {
        let provider = origin("provider-a");
        let no_search = FakeSearch::new([]);
        let attempts = FakeDriver::new([Err(TurnError::Attempt)]);
        let executor = GatewayWebSearchExecutor::new(
            &attempts,
            &no_search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::AttemptLimitExceeded
        );

        let turns = FakeDriver::new([Ok(tool_turn(provider.clone(), "turn", None))]);
        let one_result = FakeSearch::new([Ok(vec![record()])]);
        let executor = GatewayWebSearchExecutor::new(
            &turns,
            &one_result,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 2),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::TurnLimitExceeded
        );

        let tools = FakeDriver::new([
            Ok(tool_turn(provider.clone(), "tool-one", None)),
            Ok(tool_turn(provider.clone(), "tool-two", None)),
        ]);
        let one_result = FakeSearch::new([Ok(vec![record()])]);
        let executor = GatewayWebSearchExecutor::new(
            &tools,
            &one_result,
            fixed_candidate(provider.clone()),
            limits(2, 1, 2, 2),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::ToolCallLimitExceeded
        );

        let results = FakeDriver::new([Ok(tool_turn(provider.clone(), "result", None))]);
        let too_many = FakeSearch::new([Ok(vec![record(), record()])]);
        let executor = GatewayWebSearchExecutor::new(
            &results,
            &too_many,
            fixed_candidate(provider),
            limits(2, 1, 1, 2),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::ResultLimitExceeded
        );
    }

    #[tokio::test]
    async fn provenance_lifecycle_reservation_deadline_and_output_bounds_fail_closed() {
        let provider = origin("provider-a");

        let empty_driver = FakeDriver::new([]);
        let empty_search = FakeSearch::new([]);
        let executor = GatewayWebSearchExecutor::new(
            &empty_driver,
            &empty_search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(
                    request_with_origin(ToolOrigin::Downstream),
                    &CancelToken::new(),
                )
                .await
                .unwrap_err(),
            GatewayWebSearchError::InvalidToolIdentity
        );

        let failed_candidate = Candidate::new(
            CandidateId::new("candidate-failed", 64).unwrap(),
            Vec::new(),
            Some(FinishReason::Stop),
        )
        .unwrap();
        let failed_usage = Usage::new(Some(99), Some(99), Some(198), None, None);
        let failed_driver = FakeDriver::new([Ok(GatewayTurn {
            origin: provider.clone(),
            response: GenerationResponse::new(
                ResponseId::new("response-failed", 64).unwrap(),
                vec![failed_candidate],
                ResponseStatus::Failed,
                Some(failed_usage),
                Vec::new(),
            )
            .unwrap(),
            usage: Some(failed_usage),
        })]);
        let executor = GatewayWebSearchExecutor::new(
            &failed_driver,
            &empty_search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::InvalidTurnLifecycle
        );

        let mismatched_candidate = Candidate::new(
            CandidateId::new("candidate-mismatched", 64).unwrap(),
            vec![OutputItem::ToolCall(call("mismatched", web_search_input()))],
            Some(FinishReason::Stop),
        )
        .unwrap();
        let mismatched_driver = FakeDriver::new([Ok(GatewayTurn {
            origin: provider.clone(),
            response: GenerationResponse::new(
                ResponseId::new("response-mismatched", 64).unwrap(),
                vec![mismatched_candidate],
                ResponseStatus::Completed,
                None,
                Vec::new(),
            )
            .unwrap(),
            usage: None,
        })]);
        let executor = GatewayWebSearchExecutor::new(
            &mismatched_driver,
            &empty_search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::InvalidTurnLifecycle
        );

        let reserved_driver = FakeDriver::new([Ok(tool_turn(provider.clone(), "reserved", None))]);
        let reserved_search = FakeSearch::new([Ok(vec![record()])]);
        let executor = GatewayWebSearchExecutor::new(
            &reserved_driver,
            &reserved_search,
            fixed_candidate(provider.clone()),
            limits(2, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::AttemptLimitExceeded
        );
        assert_eq!(reserved_search.results.lock().unwrap().len(), 1);

        let field = "x".repeat(4 * 1024);
        let oversized_records = (0..3)
            .map(|_| SearchRecord::new(&field, &field, &field, 4 * 1024).unwrap())
            .collect::<Vec<_>>();
        let oversized_driver =
            FakeDriver::new([Ok(tool_turn(provider.clone(), "oversized", None))]);
        let oversized_search = FakeSearch::new([Ok(oversized_records)]);
        let executor = GatewayWebSearchExecutor::new(
            &oversized_driver,
            &oversized_search,
            fixed_candidate(provider.clone()),
            limits(2, 1, 3, 2),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::ResultLimitExceeded
        );

        let short_deadline =
            GatewayWebSearchLimits::new(2, 1, 1, 2, Duration::from_millis(1)).unwrap();
        let delayed_driver = DelayedDriver {
            delay: Duration::from_millis(5),
            turn: Mutex::new(Some(final_turn(provider.clone(), None))),
        };
        let executor = GatewayWebSearchExecutor::new(
            &delayed_driver,
            &empty_search,
            fixed_candidate(provider.clone()),
            short_deadline,
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::Deadline
        );

        let search_driver = FakeDriver::new([Ok(tool_turn(provider.clone(), "delayed", None))]);
        let delayed_search = DelayedSearch {
            delay: Duration::from_millis(5),
            results: Mutex::new(Some(vec![record()])),
        };
        let executor = GatewayWebSearchExecutor::new(
            &search_driver,
            &delayed_search,
            fixed_candidate(provider),
            short_deadline,
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::Deadline
        );
    }

    #[tokio::test]
    async fn cancellation_deadline_origin_and_usage_overflow_fail_closed() {
        let provider = origin("provider-a");
        let cancel = CancelToken::new();
        cancel.cancel();
        let driver = FakeDriver::new([]);
        let search = FakeSearch::new([]);
        let executor = GatewayWebSearchExecutor::new(
            &driver,
            &search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor.execute(request(), &cancel).await.unwrap_err(),
            GatewayWebSearchError::Cancelled
        );

        let raced_cancel = CancelToken::new();
        let canceling_driver = CancelingDriver {
            turn: Mutex::new(Some(final_turn(provider.clone(), None))),
        };
        let executor = GatewayWebSearchExecutor::new(
            &canceling_driver,
            &search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &raced_cancel)
                .await
                .unwrap_err(),
            GatewayWebSearchError::Cancelled
        );

        let deadline = FakeDriver::new([Err(TurnError::Deadline)]);
        let executor = GatewayWebSearchExecutor::new(
            &deadline,
            &search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::Deadline
        );

        let search_deadline_driver =
            FakeDriver::new([Ok(tool_turn(provider.clone(), "search-deadline", None))]);
        let search_deadline = FakeSearch::new([Err(SearchError::Deadline)]);
        let executor = GatewayWebSearchExecutor::new(
            &search_deadline_driver,
            &search_deadline,
            fixed_candidate(provider.clone()),
            limits(2, 1, 1, 2),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::Deadline
        );

        let wrong_origin = FakeDriver::new([Ok(final_turn(origin("provider-b"), None))]);
        let executor = GatewayWebSearchExecutor::new(
            &wrong_origin,
            &search,
            fixed_candidate(provider.clone()),
            limits(1, 1, 1, 1),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::OriginMismatch
        );

        let overflow = FakeDriver::new([
            Ok(tool_turn(
                provider.clone(),
                "overflow",
                Some(Usage::new(Some(u64::MAX), None, None, None, None)),
            )),
            Ok(final_turn(
                provider.clone(),
                Some(Usage::new(Some(1), None, None, None, None)),
            )),
        ]);
        let one_result = FakeSearch::new([Ok(vec![record()])]);
        let executor = GatewayWebSearchExecutor::new(
            &overflow,
            &one_result,
            fixed_candidate(provider),
            limits(2, 1, 1, 2),
        )
        .unwrap();
        assert_eq!(
            executor
                .execute(request(), &CancelToken::new())
                .await
                .unwrap_err(),
            GatewayWebSearchError::UsageOverflow
        );
    }
}
