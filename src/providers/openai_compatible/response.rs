//! HTTP status, SSE media, and terminal-event policy for OpenAI-compatible responses.

use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};

use crate::{
    core::ApiProtocol,
    provider::{
        ClassifiedSseEvent, RetryHint, StatusClassification, StreamEventStatus, UpstreamErrorKind,
    },
    transport::sse::SseEvent,
};

use super::{
    OpenAiCompatibleAdapter, OpenAiTerminalDiscriminator, StreamingResponseMediaTypePolicy,
};

impl OpenAiCompatibleAdapter {
    /// Identifies OpenAI-compatible Chat/Responses SSE terminal or failure events.
    pub(crate) fn classify_sse_event(
        self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> ClassifiedSseEvent {
        let status = match protocol {
            ApiProtocol::ChatCompletions if event.data() == "[DONE]" => {
                StreamEventStatus::Completed
            }
            ApiProtocol::Responses => self.classify_responses_sse_event(&event),
            _ => StreamEventStatus::Continue,
        };
        ClassifiedSseEvent::new(event, status)
    }

    /// Returns whether response headers satisfy this Provider's trusted SSE media profile.
    pub(crate) fn recognizes_sse_response(
        self,
        protocol: ApiProtocol,
        headers: &HeaderMap,
    ) -> bool {
        // Reject duplicate values before interpreting the media type.
        let mut values = headers.get_all(CONTENT_TYPE).iter();
        let Some(value) = values.next() else {
            return protocol == ApiProtocol::Responses
                && self.streaming_response_media_type_policy
                    == StreamingResponseMediaTypePolicy::AllowMissingForResponses;
        };
        if values.next().is_some() {
            return false;
        }

        // Compare only the media type token while allowing case and parameters.
        value
            .to_str()
            .ok()
            .and_then(|value| value.split(';').next())
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("text/event-stream"))
    }

    /// Identifies Responses SSE terminal states using the concrete OpenAI-compatible profile.
    fn classify_responses_sse_event(self, event: &SseEvent) -> StreamEventStatus {
        classify_openai_responses_terminal(event, self.responses_terminal_discriminator)
    }

    /// Maps an OpenAI-compatible HTTP status to an error and retry classification.
    pub(crate) fn classify_status(self, status: StatusCode) -> StatusClassification {
        // Select the error class from the OpenAI-compatible status family, then decide whether pre-output retry is allowed.
        let (kind, retry_hint) = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                (UpstreamErrorKind::InvalidRequest, RetryHint::Never)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                (UpstreamErrorKind::Authentication, RetryHint::Never)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                (UpstreamErrorKind::RateLimited, RetryHint::BeforeFirstEvent)
            }
            status if status.is_server_error() => (
                UpstreamErrorKind::UpstreamUnavailable,
                RetryHint::BeforeFirstEvent,
            ),
            _ => (UpstreamErrorKind::UpstreamFailure, RetryHint::Never),
        };
        StatusClassification::new(kind, retry_hint)
    }
}

/// Reads an OpenAI Responses terminal using the compile-time discriminator and rejects conflicting sources.
fn classify_openai_responses_terminal(
    event: &SseEvent,
    discriminator: OpenAiTerminalDiscriminator,
) -> StreamEventStatus {
    // Read the terminal source selected by the profile; remain non-terminal when it is absent.
    let (selected, corroborating) = match discriminator {
        OpenAiTerminalDiscriminator::SseEventField => {
            let selected = classify_openai_terminal_name(event.event());
            let corroborating = selected.and_then(|_| classify_data_json_openai_terminal(event));
            (selected, corroborating)
        }
        OpenAiTerminalDiscriminator::DataJsonType => (
            classify_data_json_openai_terminal(event),
            classify_openai_terminal_name(event.event()),
        ),
    };
    let Some(selected) = selected else {
        return StreamEventStatus::Continue;
    };

    // Fail closed when both explicit terminal sources conflict in one event.
    if corroborating.is_some_and(|status| status != selected) {
        StreamEventStatus::Failed
    } else {
        selected
    }
}

/// Maps an OpenAI Responses terminal name to the unified stream state.
fn classify_openai_terminal_name(name: Option<&str>) -> Option<StreamEventStatus> {
    // Convert only protocol-defined terminal names into unified lifecycle states.
    match name {
        Some("response.completed") => Some(StreamEventStatus::Completed),
        Some("response.failed" | "response.incomplete") => Some(StreamEventStatus::Failed),
        _ => None,
    }
}

/// Extracts an OpenAI Responses terminal name from the top-level `type` in data JSON.
fn classify_data_json_openai_terminal(event: &SseEvent) -> Option<StreamEventStatus> {
    // Parse the minimal event envelope without retaining or logging business content.
    let document = serde_json::from_str::<serde_json::Value>(event.data()).ok()?;
    classify_openai_terminal_name(document.get("type").and_then(serde_json::Value::as_str))
}
