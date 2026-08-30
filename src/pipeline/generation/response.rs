//! Pure response-mode decisions for one selected Generation candidate.

use super::super::types::StreamResponseConversion;

/// Response handling selected before upstream body I/O or downstream commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenerationResponseMode {
    /// Reject a successful response whose media profile cannot satisfy the plan.
    RejectInvalidMedia,
    /// Buffer one complete Responses SSE lifecycle and return its terminal JSON response.
    BufferResponsesSse,
    /// Convert upstream SSE events through the selected Generation Bridge.
    BridgeSse,
    /// Validate and transparently forward Native upstream SSE events.
    ValidateNativeSse,
    /// Decode and render one bounded successful JSON body through the canonical Generation plan.
    RenderJson,
    /// Forward one non-success upstream body without Generation decoding.
    PassthroughError,
}

/// Immutable facts needed to select Generation response handling.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GenerationResponseFacts {
    pub(crate) status_is_success: bool,
    pub(crate) downstream_streaming: bool,
    pub(crate) recognized_sse: bool,
    pub(crate) preserve_source: bool,
    pub(crate) stream_response_conversion: Option<StreamResponseConversion>,
}

/// Selects response handling without reading a body or mutating observation state.
pub(crate) fn classify_generation_response(
    facts: GenerationResponseFacts,
) -> GenerationResponseMode {
    // Successful streaming plans can consume only a response matching the trusted SSE profile.
    if facts.status_is_success
        && (facts.downstream_streaming || facts.stream_response_conversion.is_some())
        && !facts.recognized_sse
    {
        return GenerationResponseMode::RejectInvalidMedia;
    }

    // Preserve error bodies before selecting any successful-body takeover or conversion.
    if !facts.status_is_success {
        return GenerationResponseMode::PassthroughError;
    }

    // A streaming-only upstream serving a non-streaming request must complete before commit.
    if facts.stream_response_conversion == Some(StreamResponseConversion::BufferResponsesSse) {
        return GenerationResponseMode::BufferResponsesSse;
    }

    // Downstream streaming selects either cross-protocol encoding or Native source validation.
    if facts.downstream_streaming && facts.recognized_sse {
        return if facts.preserve_source {
            GenerationResponseMode::ValidateNativeSse
        } else {
            GenerationResponseMode::BridgeSse
        };
    }

    // Every successful non-streaming body is decoded by the canonical Generation plan.
    GenerationResponseMode::RenderJson
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success_facts() -> GenerationResponseFacts {
        GenerationResponseFacts {
            status_is_success: true,
            downstream_streaming: false,
            recognized_sse: false,
            preserve_source: false,
            stream_response_conversion: None,
        }
    }

    #[test]
    fn streaming_success_rejects_non_sse_media() {
        let mode = classify_generation_response(GenerationResponseFacts {
            downstream_streaming: true,
            ..success_facts()
        });

        assert_eq!(mode, GenerationResponseMode::RejectInvalidMedia);
    }

    #[test]
    fn streaming_takeover_rejects_non_sse_media() {
        let mode = classify_generation_response(GenerationResponseFacts {
            stream_response_conversion: Some(StreamResponseConversion::BufferResponsesSse),
            ..success_facts()
        });

        assert_eq!(mode, GenerationResponseMode::RejectInvalidMedia);
    }

    #[test]
    fn streaming_takeover_uses_canonical_buffering() {
        let mode = classify_generation_response(GenerationResponseFacts {
            recognized_sse: true,
            stream_response_conversion: Some(StreamResponseConversion::BufferResponsesSse),
            ..success_facts()
        });

        assert_eq!(mode, GenerationResponseMode::BufferResponsesSse);
    }

    #[test]
    fn streaming_bridge_uses_event_conversion() {
        let mode = classify_generation_response(GenerationResponseFacts {
            downstream_streaming: true,
            recognized_sse: true,
            ..success_facts()
        });

        assert_eq!(mode, GenerationResponseMode::BridgeSse);
    }

    #[test]
    fn streaming_native_uses_sse_validation() {
        let mode = classify_generation_response(GenerationResponseFacts {
            downstream_streaming: true,
            recognized_sse: true,
            preserve_source: true,
            ..success_facts()
        });

        assert_eq!(mode, GenerationResponseMode::ValidateNativeSse);
    }

    #[test]
    fn non_streaming_success_uses_canonical_json_rendering() {
        let mode = classify_generation_response(success_facts());

        assert_eq!(mode, GenerationResponseMode::RenderJson);
    }

    #[test]
    fn upstream_error_body_remains_passthrough() {
        let mode = classify_generation_response(GenerationResponseFacts {
            status_is_success: false,
            downstream_streaming: true,
            stream_response_conversion: Some(StreamResponseConversion::BufferResponsesSse),
            ..success_facts()
        });

        assert_eq!(mode, GenerationResponseMode::PassthroughError);
    }
}
